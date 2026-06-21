use crate::font::FontHandle;
use crate::parsing::Color;
use crate::ui_shit::layout;
use crate::ui_shit::paint::{BorderStyle, DecodedImage, DisplayCommand, DisplayList, TextRange};

fn argb(c: &Color) -> u32 {
    (c.3 as u32) << 24 | (c.0 as u32) << 16 | (c.1 as u32) << 8 | (c.2 as u32)
}

pub struct CpuRenderer {
    pub buffer: Vec<u32>,
    pub width: u32,
    pub height: u32,
    font: Option<FontHandle>,
    clip_stack: Vec<layout::Rect>,
    global_alpha: f32,
}

fn load_font() -> Option<FontHandle> {
    let paths = [
        crate::font::DEFAULT_FONT_PATH,
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    for p in &paths {
        if let Ok(data) = std::fs::read(p) {
            let b = data.into_boxed_slice();
            let f = FontHandle::load(b, 16.0);
            if f.is_some() {
                return f;
            }
        }
    }
    None
}

impl CpuRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let buffer = vec![0xFFFFFFFF; (width * height) as usize];
        let font = load_font();
        CpuRenderer { buffer, width, height, font, clip_stack: Vec::new(), global_alpha: 1.0 }
    }

    fn clip_intersect(rect: &layout::Rect, clip_stack: &[layout::Rect]) -> Option<layout::Rect> {
        if clip_stack.is_empty() {
            return Some(*rect);
        }
        let clip = clip_stack.last().unwrap();
        let x1 = rect.x.max(clip.x);
        let y1 = rect.y.max(clip.y);
        let x2 = (rect.x + rect.width).min(clip.x + clip.width);
        let y2 = (rect.y + rect.height).min(clip.y + clip.height);
        if x1 >= x2 || y1 >= y2 {
            return None;
        }
        Some(layout::Rect { x: x1, y: y1, width: x2 - x1, height: y2 - y1 })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w != self.width || h != self.height {
            self.width = w;
            self.height = h;
            self.buffer.resize((w * h) as usize, 0xFFFFFFFF);
        }
    }

    pub fn render(&mut self, list: &DisplayList) {
        self.buffer.fill(0xFFFFFFFF);
        for cmd in &list.items {
            self.render_cmd(cmd, &list.text_arena, &list.decoded_images);
        }
    }

    fn render_cmd(&mut self, cmd: &DisplayCommand, text_arena: &str, images: &[DecodedImage]) {
        match cmd {
            DisplayCommand::FillRect(r, c) => self.fill_rect_clipped(r, c),
            DisplayCommand::FillGradient(r, g) => self.fill_gradient_clipped(r, g),
            DisplayCommand::Border(rect, sides) => {
                let r = *rect;
                if sides[0].width > 0.0 && sides[0].style != BorderStyle::None {
                    self.fill_rect_clipped(&layout::Rect { x: r.x, y: r.y, width: r.width, height: sides[0].width }, &sides[0].color);
                }
                if sides[1].width > 0.0 && sides[1].style != BorderStyle::None {
                    self.fill_rect_clipped(&layout::Rect { x: r.x + r.width - sides[1].width, y: r.y, width: sides[1].width, height: r.height }, &sides[1].color);
                }
                if sides[2].width > 0.0 && sides[2].style != BorderStyle::None {
                    self.fill_rect_clipped(&layout::Rect { x: r.x, y: r.y + r.height - sides[2].width, width: r.width, height: sides[2].width }, &sides[2].color);
                }
                if sides[3].width > 0.0 && sides[3].style != BorderStyle::None {
                    self.fill_rect_clipped(&layout::Rect { x: r.x, y: r.y, width: sides[3].width, height: r.height }, &sides[3].color);
                }
            }
            DisplayCommand::DrawBoxShadow(r, c, _ox, _oy, _bl) => {
                let alpha = (c.3 as f32 * 0.5 * self.global_alpha).min(255.0) as u8;
                let sc = Color(c.0, c.1, c.2, alpha);
                self.fill_rect_clipped(r, &sc);
            }
            DisplayCommand::DrawImage(r, idx) => {
                if let Some(img) = images.get(*idx as usize) {
                    self.draw_image_clipped(r, img);
                }
            }
            DisplayCommand::TextRun(r, c, _fz, _ff, range) => {
                let ca = Color(c.0, c.1, c.2, (c.3 as f32 * self.global_alpha).min(255.0) as u8);
                self.text_run(r, &ca, range, text_arena);
            }
            DisplayCommand::SetClip(rect) => {
                self.clip_stack.push(*rect);
            }
            DisplayCommand::PopClip => {
                self.clip_stack.pop();
            }
            DisplayCommand::SetOpacity(val) => {
                self.global_alpha = *val;
            }
            DisplayCommand::PopOpacity => {
                self.global_alpha = 1.0;
            }
        }
    }

    fn fill_gradient(&mut self, rect: &layout::Rect, grad: &crate::ui_shit::paint::Gradient) {
        let x0 = rect.x.max(0.0) as u32;
        let y0 = rect.y.max(0.0) as u32;
        let x1 = (rect.x + rect.width).min(self.width as f32) as u32;
        let y1 = (rect.y + rect.height).min(self.height as f32) as u32;

        let from = grad.from;
        let to = grad.to;

        for y in y0..y1 {
            let row = (y * self.width) as usize;
            for x in x0..x1 {
                let t = if grad.vertical {
                    if rect.height <= 0.0 { 0.0 } else { (y as f32 - rect.y) / rect.height }
                } else {
                    if rect.width <= 0.0 { 0.0 } else { (x as f32 - rect.x) / rect.width }
                };
                let t = t.clamp(0.0, 1.0);
                let inv = 1.0 - t;
                let r = (from.0 as f32 * inv + to.0 as f32 * t) as u32;
                let g = (from.1 as f32 * inv + to.1 as f32 * t) as u32;
                let b = (from.2 as f32 * inv + to.2 as f32 * t) as u32;
                let a = (from.3 as f32 * inv + to.3 as f32 * t) as u32;
                self.buffer[row + x as usize] = (a.min(255) << 24) | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
            }
        }
    }

    fn fill_gradient_clipped(&mut self, rect: &layout::Rect, grad: &crate::ui_shit::paint::Gradient) {
        if let Some(r) = Self::clip_intersect(rect, &self.clip_stack) {
            self.fill_gradient(&r, grad);
        }
    }

    fn draw_image(&mut self, rect: &layout::Rect, img: &DecodedImage) {
        let dx0 = rect.x.max(0.0) as u32;
        let dy0 = rect.y.max(0.0) as u32;
        let dx1 = (rect.x + rect.width).min(self.width as f32) as u32;
        let dy1 = (rect.y + rect.height).min(self.height as f32) as u32;

        for dy in dy0..dy1 {
            let row = (dy * self.width) as usize;
            let src_y = ((dy as f32 - rect.y) / rect.height * img.height as f32) as u32;
            let src_row = src_y.min(img.height - 1) as usize * img.width as usize * 4;

            for dx in dx0..dx1 {
                let src_x = ((dx as f32 - rect.x) / rect.width * img.width as f32) as u32;
                let src_idx = src_row + src_x.min(img.width - 1) as usize * 4;
                let r = img.rgba[src_idx] as u32;
                let g = img.rgba[src_idx + 1] as u32;
                let b = img.rgba[src_idx + 2] as u32;
                let a = img.rgba[src_idx + 3] as u32;

                if a == 255 {
                    self.buffer[row + dx as usize] = (0xFF << 24) | (r << 16) | (g << 8) | b;
                } else if a > 0 {
                    let dst = self.buffer[row + dx as usize];
                    let inv = 255 - a;
                    let dr = (dst >> 16) & 0xFF;
                    let dg = (dst >> 8) & 0xFF;
                    let db = dst & 0xFF;
                    self.buffer[row + dx as usize] = (0xFF << 24)
                        | (((r * a + dr * inv) / 255) << 16)
                        | (((g * a + dg * inv) / 255) << 8)
                        | ((b * a + db * inv) / 255);
                }
            }
        }
    }

    fn draw_image_clipped(&mut self, rect: &layout::Rect, img: &DecodedImage) {
        if let Some(r) = Self::clip_intersect(rect, &self.clip_stack) {
            self.draw_image(&r, img);
        }
    }

    fn fill_rect(&mut self, rect: &crate::ui_shit::layout::Rect, color: &Color) {
        let p = argb(color);
        let x0 = rect.x.max(0.0) as u32;
        let y0 = rect.y.max(0.0) as u32;
        let x1 = (rect.x + rect.width).min(self.width as f32) as u32;
        let y1 = (rect.y + rect.height).min(self.height as f32) as u32;
        for y in y0..y1 {
            let row = (y * self.width) as usize;
            for x in x0..x1 {
                self.buffer[row + x as usize] = p;
            }
        }
    }

    fn fill_rect_clipped(&mut self, rect: &layout::Rect, color: &Color) {
        if let Some(r) = Self::clip_intersect(rect, &self.clip_stack) {
            self.fill_rect(&r, color);
        }
    }

    fn text_run(
        &mut self,
        rect: &crate::ui_shit::layout::Rect,
        color: &Color,
        range: &TextRange,
        text_arena: &str,
    ) {
        let font = match &self.font {
            Some(f) => f,
            None => return,
        };

        let text = &text_arena[range.start as usize..][..range.len as usize];
        let cr = color.0 as u32;
        let cg = color.1 as u32;
        let cb = color.2 as u32;

        let (glyphs, bitmap) = match font.fill_glyphs(text) {
            Some(v) => v,
            None => return,
        };

        // approximate baseline: ~85% down the layout rect
        let baseline_y = rect.y + rect.height * 0.85;

        let mut cursor_x = rect.x;
        for info in &glyphs {
            cursor_x += info.ker_x;

            let x0 = (cursor_x + info.br_x) as i32;
            let y0 = (baseline_y - info.br_y) as i32;
            let bw = info.bm_width as i32;
            let bh = info.bm_rows as i32;
            let pitch = info.bm_pitch as usize;

            for row in 0..bh {
                let sy = y0 + row;
                if sy < 0 || sy as u32 >= self.height {
                    continue;
                }
                let row_start = (sy as u32 * self.width) as usize;
                for col in 0..bw {
                    let sx = x0 + col;
                    if sx < 0 || sx as u32 >= self.width {
                        continue;
                    }
                    // clip check
                    if let Some(cr) = self.clip_stack.last() {
                        let px = sx as f32;
                        let py = sy as f32;
                        if px < cr.x || px >= cr.x + cr.width || py < cr.y || py >= cr.y + cr.height {
                            continue;
                        }
                    }
                    let cov = bitmap[(info.bm_offset as usize) + row as usize * pitch + col as usize] as u32;
                    if cov == 0 {
                        continue;
                    }
                    let dst = self.buffer[row_start + sx as usize];
                    let a = cov;
                    if a == 255 {
                        self.buffer[row_start + sx as usize] = (0xFF << 24) | (cr << 16) | (cg << 8) | cb;
                    } else {
                        let inv = 255 - a;
                        let dr = (dst >> 16) & 0xFF;
                        let dg = (dst >> 8) & 0xFF;
                        let db = dst & 0xFF;
                        let r = (cr * a + dr * inv) / 255;
                        let g = (cg * a + dg * inv) / 255;
                        let b = (cb * a + db * inv) / 255;
                        self.buffer[row_start + sx as usize] = (0xFF << 24) | (r << 16) | (g << 8) | b;
                    }
                }
            }

            cursor_x += info.adv_x;
        }
    }
}
