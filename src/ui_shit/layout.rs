use crate::parsing::{ComputedValues, Display, Length, Position, Overflow, Float};
use crate::parsing::RenderNode;

#[cfg(freetype_avail)]
use std::sync::OnceLock;
#[cfg(freetype_avail)]
use crate::font::FontHandle;

// --- Geometry types ---

#[derive(Debug, Clone, Copy)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// --- Float tracking ---

#[derive(Debug, Clone, Copy)]
struct FloatBox {
    rect: Rect,
    float: Float,
}

// --- Layout box ---

#[derive(Debug)]
pub struct LayoutBox {
    pub tag: String,
    pub text: String,
    pub style: ComputedValues,
    pub rect: Rect,
    pub children: Vec<LayoutBox>,
    pub positioned_children: Vec<LayoutBox>,
    pub clip_rect: Option<Rect>,
}

impl LayoutBox {
    fn new(tag: String, text: String, style: ComputedValues, rect: Rect) -> Self {
        LayoutBox {
            tag,
            text,
            style,
            rect,
            children: vec![],
            positioned_children: vec![],
            clip_rect: None,
        }
    }
}

// --- Containing block context ---

#[derive(Debug, Clone, Copy)]
struct CbContext {
    rect: Rect,
    is_positioned: bool,
}

// --- Layout engine trait ---

pub trait LayoutEngine {
    fn layout(&self, root: &RenderNode, viewport: Size) -> Vec<LayoutBox>;
}

// --- Default block layout engine ---

pub struct BlockLayout;

impl LayoutEngine for BlockLayout {
    fn layout(&self, root: &RenderNode, viewport: Size) -> Vec<LayoutBox> {
        let mut boxes = Vec::new();
        let mut cursor = Vec2 { x: 0.0, y: 0.0 };
        let viewport_rect = Rect { x: 0.0, y: 0.0, width: viewport.width, height: viewport.height };
        let cb = CbContext { rect: viewport_rect, is_positioned: true };
        let mut floats: Vec<FloatBox> = Vec::new();
        layout_children(root, &viewport_rect, &cb, &mut cursor, &mut boxes, &mut floats);
        boxes
    }
}

// --- Main layout dispatch ---

#[allow(clippy::too_many_arguments)]
fn layout_children(
    node: &RenderNode,
    containing: &Rect,
    cb: &CbContext,
    cursor: &mut Vec2,
    out: &mut Vec<LayoutBox>,
    floats: &mut Vec<FloatBox>,
) {
    if node.style.display == Display::None {
        return;
    }

    match node.style.position {
        Position::Absolute | Position::Fixed => {
            let box_ = layout_positioned(node, containing, cb);
            out.push(box_);
            return;
        }
        _ => {}
    }

    if !is_floating(&node.style) {
        match node.style.display {
            Display::Block => {
                let box_ = layout_block(node, containing, cb, cursor, floats);
                out.push(box_);
            }
            Display::Inline | _ => {
                let mut inline_children = Vec::new();
                collect_inline(node, &mut inline_children);
                let boxes = layout_inlines(&inline_children, containing, cursor, floats);
                out.extend(boxes);
            }
        }
    } else {
        let box_ = layout_float(node, containing, cb, cursor, floats);
        out.push(box_);
    }
}

// --- Positioned element layout ---

fn layout_positioned(node: &RenderNode, _containing: &Rect, cb: &CbContext) -> LayoutBox {
    let cb_rect = cb.rect;
    let style = &node.style;

    let w = resolve_length(&style.width, cb_rect.width);
    let h = resolve_length(&style.height, cb_rect.height);

    let cb_w = if w > 0.0 { w } else { cb_rect.width };
    let cb_h = if h > 0.0 { h } else { 0.0 };

    let x = match style.left {
        Length::Px(l) => cb_rect.x + l - (if w > 0.0 { 0.0 } else { resolve_length(&style.right, cb_rect.width) }),
        _ => match style.right {
            Length::Px(r) => cb_rect.x + cb_rect.width - r - cb_w,
            _ => cb_rect.x,
        }
    };

    let y = match style.top {
        Length::Px(t) => cb_rect.y + t,
        _ => match style.bottom {
            Length::Px(b) => cb_rect.y + cb_rect.height - b - cb_h,
            _ => cursor_for_positioned(node, cb),
        }
    };

    let child_cb = CbContext {
        rect: Rect { x: 0.0, y: 0.0, width: cb_w, height: cb_h },
        is_positioned: true,
    };

    let mut children = Vec::new();
    let mut positioned = Vec::new();
    let mut floats = Vec::new();
    let mut child_cursor = Vec2 { x: 0.0, y: 0.0 };

    for child in &node.children {
        if child.style.position == Position::Absolute || child.style.position == Position::Fixed {
            let pc = layout_positioned(child, &child_cb.rect, &child_cb);
            positioned.push(pc);
            continue;
        }
        if child.tag == "#text" || child.style.display == Display::Inline || is_floating(&child.style) {
            let mut inline_collected = Vec::new();
            collect_inline(child, &mut inline_collected);
            let ib = layout_inlines(&inline_collected, &child_cb.rect, &mut child_cursor, &mut floats);
            children.extend(ib);
        } else if child.style.display == Display::Block {
            let cb = layout_block(child, &child_cb.rect, &child_cb, &mut child_cursor, &mut floats);
            children.push(cb);
        }
    }

    let content_h = child_cursor.y;
    let box_h = if h > 0.0 { h } else { content_h };

    let clip = if style.overflow_x != Overflow::Visible || style.overflow_y != Overflow::Visible {
        Some(Rect { x: 0.0, y: 0.0, width: cb_w, height: box_h })
    } else {
        None
    };

    let mut box_ = LayoutBox::new(
        node.tag.clone(),
        node.text.clone(),
        node.style.clone(),
        Rect { x, y, width: cb_w, height: box_h },
    );
    box_.children = children;
    box_.positioned_children = positioned;
    box_.clip_rect = clip;
    box_
}

fn cursor_for_positioned(node: &RenderNode, cb: &CbContext) -> f32 {
    let mut y = cb.rect.y;
    for child in &node.children {
        if child.tag != "#text" && child.style.display == Display::Block {
            let m_t = resolve_length(&child.style.margin_top, cb.rect.width);
            y += m_t;
            let h = resolve_length(&child.style.height, cb.rect.width);
            let m_b = resolve_length(&child.style.margin_bottom, cb.rect.width);
            y += h + m_b;
        }
    }
    y
}

// --- Float layout ---

fn is_floating(style: &ComputedValues) -> bool {
    style.float != Float::None
}

fn layout_float(node: &RenderNode, containing: &Rect, cb: &CbContext, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) -> LayoutBox {
    let m_t = resolve_length(&node.style.margin_top, containing.width);
    let m_r = resolve_length(&node.style.margin_right, containing.width);
    let m_b = resolve_length(&node.style.margin_bottom, containing.width);
    let m_l = resolve_length(&node.style.margin_left, containing.width);

    let p_t = resolve_length(&node.style.padding_top, containing.width);
    let p_r = resolve_length(&node.style.padding_right, containing.width);
    let p_b = resolve_length(&node.style.padding_bottom, containing.width);
    let p_l = resolve_length(&node.style.padding_left, containing.width);

    let w = resolve_length(&node.style.width, containing.width);
    let h = resolve_length(&node.style.height, containing.width);

    let float_w = if w > 0.0 { w + m_l + m_r + p_l + p_r } else { containing.width * 0.5 };

    let float_x = match node.style.float {
        Float::Left => containing.x + m_l,
        _ => containing.x + containing.width - float_w + m_l,
    };

    let float_y = cursor.y + m_t;

    let inner_w = if w > 0.0 { w - p_l - p_r } else { float_w - m_l - m_r - p_l - p_r };
    let content_x = float_x + p_l;
    let content_y = float_y + p_t;

    let mut children = Vec::new();
    let mut child_cursor = Vec2 { x: content_x, y: content_y };
    let content_rect = Rect { x: content_x, y: content_y, width: inner_w, height: 0.0 };

    for child in &node.children {
        if child.tag == "#text" || child.style.display == Display::Inline {
            let mut inline_collected = Vec::new();
            collect_inline(child, &mut inline_collected);
            let ib = layout_inlines(&inline_collected, &content_rect, &mut child_cursor, floats);
            children.extend(ib);
        } else if child.style.display == Display::Block {
            let cb = layout_block(child, &content_rect, cb, &mut child_cursor, floats);
            children.push(cb);
        }
    }

    let content_h = child_cursor.y - content_y;
    let box_h = if h > 0.0 { h } else { content_h + p_t + p_b };
    cursor.y = float_y + box_h + m_b;

    let rect = Rect { x: float_x, y: float_y, width: float_w, height: box_h };
    floats.push(FloatBox { rect, float: node.style.float });

    LayoutBox::new(node.tag.clone(), node.text.clone(), node.style.clone(), rect)
}

// --- Block layout ---

fn layout_block(node: &RenderNode, containing: &Rect, _cb: &CbContext, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) -> LayoutBox {
    let m_t = resolve_length(&node.style.margin_top, containing.width);
    let m_r = resolve_length(&node.style.margin_right, containing.width);
    let m_b = resolve_length(&node.style.margin_bottom, containing.width);
    let m_l = resolve_length(&node.style.margin_left, containing.width);

    let p_t = resolve_length(&node.style.padding_top, containing.width);
    let p_r = resolve_length(&node.style.padding_right, containing.width);
    let p_b = resolve_length(&node.style.padding_bottom, containing.width);
    let p_l = resolve_length(&node.style.padding_left, containing.width);

    let w = resolve_length(&node.style.width, containing.width);
    let h = resolve_length(&node.style.height, containing.width);

    let content_w = containing.width - m_l - m_r;
    let inner_w = if w > 0.0 { w - p_l - p_r } else { content_w - p_l - p_r };

    let x = containing.x + m_l;
    let y = cursor.y + m_t;

    let content_x = x + p_l;
    let content_y = y + p_t;

    let mut children: Vec<LayoutBox> = Vec::new();
    let mut positioned: Vec<LayoutBox> = Vec::new();
    let mut child_cursor = Vec2 { x: content_x, y: content_y };
    let mut inline_batch: Vec<&RenderNode> = Vec::new();
    let content_rect = Rect { x: content_x, y: content_y, width: inner_w, height: 0.0 };

    let child_cb = if node.style.position != Position::Static {
        CbContext { rect: content_rect, is_positioned: true }
    } else {
        CbContext { rect: content_rect, is_positioned: false }
    };

    for child in &node.children {
        match child.style.position {
            Position::Absolute | Position::Fixed => {
                flush_inlines(&mut inline_batch, &mut children, &content_rect, &mut child_cursor, floats);
                let pc = layout_positioned(child, &content_rect, &child_cb);
                positioned.push(pc);
                continue;
            }
            _ => {}
        }

        if child.tag == "#text" {
            inline_batch.push(child);
            continue;
        }

        match &child.style.display {
            Display::Block => {
                flush_inlines(&mut inline_batch, &mut children, &content_rect, &mut child_cursor, floats);
                let cb = layout_block(child, &content_rect, &child_cb, &mut child_cursor, floats);
                children.push(cb);
            }
            _ => {
                let mut inline_collected = Vec::new();
                collect_inline(child, &mut inline_collected);
                inline_batch.extend(inline_collected);
            }
        }
    }

    flush_inlines(&mut inline_batch, &mut children, &content_rect, &mut child_cursor, floats);

    let content_h = child_cursor.y - content_y;
    let box_h = if h > 0.0 { h } else { content_h + p_t + p_b };
    cursor.y = y + box_h + m_b;

    let clip = if node.style.overflow_x != Overflow::Visible || node.style.overflow_y != Overflow::Visible {
        Some(Rect { x, y, width: content_w, height: box_h })
    } else {
        None
    };

    let mut box_ = LayoutBox::new(
        node.tag.clone(),
        node.text.clone(),
        node.style.clone(),
        Rect { x, y, width: content_w, height: box_h },
    );
    box_.children = children;
    box_.positioned_children = positioned;
    box_.clip_rect = clip;
    box_
}

// --- Inline layout ---

fn collect_inline<'a>(node: &'a RenderNode, out: &mut Vec<&'a RenderNode>) {
    if node.tag == "#text" {
        out.push(node);
    } else {
        for child in &node.children {
            collect_inline(child, out);
        }
    }
}

fn layout_inlines(nodes: &[&RenderNode], containing: &Rect, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    let mut line_x = containing.x;

    let available_width = available_inline_width(containing.x, containing.x + containing.width, cursor.y, floats);

    for node in nodes {
        if node.tag == "#text" {
            let text_w = estimate_text_width(&node.text, node.style.font_size);
            if line_x + text_w > available_width && line_x > containing.x {
                cursor.y += node.style.font_size * 1.2;
                line_x = containing.x;
            }
            boxes.push(LayoutBox::new(
                "#text".into(),
                node.text.clone(),
                node.style.clone(),
                Rect { x: line_x, y: cursor.y, width: text_w, height: node.style.font_size * 1.2 },
            ));
            line_x += text_w;
        }
    }

    if !nodes.is_empty() {
        cursor.y += nodes.last().unwrap().style.font_size * 1.2;
    }

    boxes
}

fn flush_inlines<'a>(batch: &mut Vec<&'a RenderNode>, children: &mut Vec<LayoutBox>, content_rect: &Rect, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) {
    if !batch.is_empty() {
        let nodes: Vec<&RenderNode> = batch.drain(..).collect();
        let lbs = layout_inlines(&nodes, content_rect, cursor, floats);
        children.extend(lbs);
    }
}

fn available_inline_width(container_left: f32, container_right: f32, y: f32, floats: &[FloatBox]) -> f32 {
    let mut left = container_left;
    for f in floats {
        if y >= f.rect.y && y < f.rect.y + f.rect.height {
            match f.float {
                Float::Left => left = left.max(f.rect.x + f.rect.width),
                Float::Right => return (container_right - left).max(0.0),
                _ => {}
            }
        }
    }
    (container_right - left).max(0.0)
}

// --- Font ---

#[cfg(freetype_avail)]
fn font_cache() -> Option<&'static FontHandle> {
    static FONT: OnceLock<Option<FontHandle>> = OnceLock::new();
    FONT.get_or_init(|| {
        let paths = [
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ];
        for path in &paths {
            if let Ok(data) = std::fs::read(path) {
                let boxed = data.into_boxed_slice();
                if let Some(font) = FontHandle::load(boxed, 16.0) {
                    return Some(font);
                }
            }
        }
        None
    })
    .as_ref()
}

fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    #[cfg(freetype_avail)]
    if let Some(font) = font_cache() {
        return font.measure(text) * (font_size / 16.0);
    }

    let char_w = font_size * 0.6;
    text.len() as f32 * char_w
}

// --- Helpers ---

pub fn resolve_length(length: &Length, _parent_width: f32) -> f32 {
    match length {
        Length::Px(v) => *v,
        Length::Auto => 0.0,
    }
}

// --- Layout tree dump ---

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn dump_positioned(boxes: &[LayoutBox], depth: usize, prefix: &str) {
    for box_ in boxes {
        let indent = "  ".repeat(depth);
        if box_.tag == "#text" {
            let display = if box_.text.chars().count() > 40 {
                format!("{}…", truncate(&box_.text, 40))
            } else {
                box_.text.clone()
            };
            println!("{}{}text {:>8.1},{:>4.1} {:>6.1}x{:<4.1} '{}'",
                indent, prefix, box_.rect.x, box_.rect.y, box_.rect.width, box_.rect.height, display);
        } else {
            println!("{}{}<{}> {:>8.1},{:>4.1} {:>6.1}x{:<4.1}",
                indent, prefix, box_.tag, box_.rect.x, box_.rect.y, box_.rect.width, box_.rect.height);
            for child in &box_.children {
                dump_box(child, depth + 1);
            }
            if !box_.positioned_children.is_empty() {
                dump_positioned(&box_.positioned_children, depth + 1, "[pos] ");
            }
        }
    }
}

pub fn dump_box(box_: &LayoutBox, depth: usize) {
    let indent = "  ".repeat(depth);
    if box_.tag == "#text" {
        let display = if box_.text.chars().count() > 40 {
            format!("{}…", truncate(&box_.text, 40))
        } else {
            box_.text.clone()
        };
        println!("{}text {:>8.1},{:>4.1} {:>6.1}x{:<4.1} '{}'",
            indent, box_.rect.x, box_.rect.y, box_.rect.width, box_.rect.height, display);
    } else {
        let clip = if let Some(cr) = box_.clip_rect {
            format!(" clip=({:.0},{:.0} {:.0}x{:.0})", cr.x, cr.y, cr.width, cr.height)
        } else {
            String::new()
        };
        println!("{}<{}> {:>8.1},{:>4.1} {:>6.1}x{:<4.1}{}",
            indent, box_.tag, box_.rect.x, box_.rect.y, box_.rect.width, box_.rect.height, clip);
        for child in &box_.children {
            dump_box(child, depth + 1);
        }
        if !box_.positioned_children.is_empty() {
            dump_positioned(&box_.positioned_children, depth + 1, "[pos] ");
        }
    }
}

pub fn dump_boxes(boxes: &[LayoutBox]) {
    for box_ in boxes {
        dump_box(box_, 0);
    }
}
