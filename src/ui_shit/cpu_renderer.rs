use crate::font::FontHandle;
use crate::parsing::Color;
use crate::ui_shit::layout;
use crate::ui_shit::paint::{DisplayCommand, DisplayList, TextRange};

fn argb(c: &Color) -> u32 {
    (c.3 as u32) << 24 | (c.0 as u32) << 16 | (c.1 as u32) << 8 | (c.2 as u32)
}

pub struct CpuRenderer {
    pub buffer: Vec<u32>,
    pub width: u32,
    pub height: u32,
    font: Option<FontHandle>,
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
        CpuRenderer { buffer, width, height, font }
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
            self.render_cmd(cmd, &list.text_arena);
        }
    }

    fn render_cmd(&mut self, cmd: &DisplayCommand, text_arena: &str) {
        match cmd {
            DisplayCommand::FillRect(r, c) => self.fill_rect(r, c),
            DisplayCommand::FillGradient(r, g) => {
                let c = Color((g.from.0 + g.to.0) / 2, (g.from.1 + g.to.1) / 2, (g.from.2 + g.to.2) / 2, 200);
                self.fill_rect(r, &c);
            }
            DisplayCommand::Border(rect, sides) => {
                let r = *rect;
                self.fill_rect(&layout::Rect { x: r.x, y: r.y, width: r.width, height: sides[0].width }, &sides[0].color);
                self.fill_rect(&layout::Rect { x: r.x + r.width - sides[1].width, y: r.y, width: sides[1].width, height: r.height }, &sides[1].color);
                self.fill_rect(&layout::Rect { x: r.x, y: r.y + r.height - sides[2].width, width: r.width, height: sides[2].width }, &sides[2].color);
                self.fill_rect(&layout::Rect { x: r.x, y: r.y, width: sides[3].width, height: r.height }, &sides[3].color);
            }
            DisplayCommand::DrawBoxShadow(r, c, _ox, _oy, _bl) => {
                let sc = Color(c.0, c.1, c.2, (c.3 as f32 * 0.5) as u8);
                self.fill_rect(r, &sc);
            }
            DisplayCommand::TextRun(r, c, _fz, _ff, range) => {
                self.text_run(r, c, range, text_arena);
            }
            _ => {}
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
