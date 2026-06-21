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

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub tag: String,
    pub text: String,
    pub style: ComputedValues,
    pub rect: Rect,
    pub children: Vec<LayoutBox>,
    pub positioned_children: Vec<LayoutBox>,
    pub clip_rect: Option<Rect>,
    pub src: Option<String>,
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
            src: None,
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
                let mut prev_margin = 0.0;
                let box_ = layout_block(node, containing, cb, cursor, floats, &mut prev_margin);
                out.push(box_);
            }
            Display::Inline => {
                let mut inline_children = Vec::new();
                collect_inline(node, &mut inline_children);
                let items: Vec<InlineItem> = inline_children.into_iter().map(|n| InlineItem::Text(n.text.clone(), n.style.clone())).collect();
                let boxes = layout_inlines(items, containing, cursor, floats);
                out.extend(boxes);
            }
            Display::InlineBlock => {
                let mut ib_cursor = Vec2 { x: containing.x, y: cursor.y };
                let mut zero_margin = 0.0;
                let ib_box = layout_block(node, containing, cb, &mut ib_cursor, floats, &mut zero_margin);
                out.push(ib_box);
            }
            _ => {
                let mut prev_margin = 0.0;
                let box_ = layout_block(node, containing, cb, cursor, floats, &mut prev_margin);
                out.push(box_);
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
    let mut child_prev_margin = 0.0;

    for child in &node.children {
        if child.style.position == Position::Absolute || child.style.position == Position::Fixed {
            let pc = layout_positioned(child, &child_cb.rect, &child_cb);
            positioned.push(pc);
            continue;
        }
        if child.tag == "#text" {
            let items = vec![InlineItem::Text(child.text.clone(), child.style.clone())];
            let ib = layout_inlines(items, &child_cb.rect, &mut child_cursor, &mut floats);
            children.extend(ib);
        } else if child.style.display == Display::Inline || is_floating(&child.style) {
            let mut inline_collected = Vec::new();
            collect_inline(child, &mut inline_collected);
            let items: Vec<InlineItem> = inline_collected.into_iter().map(|n| InlineItem::Text(n.text.clone(), n.style.clone())).collect();
            let ib = layout_inlines(items, &child_cb.rect, &mut child_cursor, &mut floats);
            children.extend(ib);
        } else if child.style.display == Display::InlineBlock {
            let mut ib_cursor = Vec2 { x: child_cb.rect.x, y: child_cursor.y };
            let mut zero_margin = 0.0;
            let ib_box = layout_block(child, &child_cb.rect, &child_cb, &mut ib_cursor, &mut floats, &mut zero_margin);
            children.push(ib_box);
        } else if child.style.display == Display::Block {
            let cb = layout_block(child, &child_cb.rect, &child_cb, &mut child_cursor, &mut floats, &mut child_prev_margin);
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
    box_.src = node.src.clone();
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
    let mut child_prev_margin = 0.0;

    for child in &node.children {
        if child.tag == "#text" {
            let items = vec![InlineItem::Text(child.text.clone(), child.style.clone())];
            let ib = layout_inlines(items, &content_rect, &mut child_cursor, floats);
            children.extend(ib);
        } else if child.style.display == Display::Inline {
            let mut inline_collected = Vec::new();
            collect_inline(child, &mut inline_collected);
            let items: Vec<InlineItem> = inline_collected.into_iter().map(|n| InlineItem::Text(n.text.clone(), n.style.clone())).collect();
            let ib = layout_inlines(items, &content_rect, &mut child_cursor, floats);
            children.extend(ib);
        } else if child.style.display == Display::InlineBlock {
            let mut ib_cursor = Vec2 { x: content_rect.x, y: child_cursor.y };
            let mut zero_margin = 0.0;
            let ib_box = layout_block(child, &content_rect, cb, &mut ib_cursor, floats, &mut zero_margin);
            children.push(ib_box);
        } else if child.style.display == Display::Block {
            let cb = layout_block(child, &content_rect, cb, &mut child_cursor, floats, &mut child_prev_margin);
            children.push(cb);
        }
    }

    let content_h = child_cursor.y - content_y;
    let box_h = if h > 0.0 { h } else { content_h + p_t + p_b };
    cursor.y = float_y + box_h + m_b;

    let mut rect = Rect { x: float_x, y: float_y, width: float_w, height: box_h };
    if node.style.position == Position::Relative {
        rect.x += resolve_length(&node.style.left, containing.width) - resolve_length(&node.style.right, containing.width);
        rect.y += resolve_length(&node.style.top, containing.width) - resolve_length(&node.style.bottom, containing.width);
    }
    floats.push(FloatBox { rect, float: node.style.float });

    let mut box_ = LayoutBox::new(node.tag.clone(), node.text.clone(), node.style.clone(), rect);
    box_.src = node.src.clone();
    box_
}

// --- Block layout ---

fn layout_block(node: &RenderNode, containing: &Rect, _cb: &CbContext, cursor: &mut Vec2, floats: &mut Vec<FloatBox>, prev_margin: &mut f32) -> LayoutBox {
    let m_t = resolve_length(&node.style.margin_top, containing.width);
    let mut m_r = resolve_length(&node.style.margin_right, containing.width);
    let m_b = resolve_length(&node.style.margin_bottom, containing.width);
    let mut m_l = resolve_length(&node.style.margin_left, containing.width);

    let gap = m_t.max(*prev_margin);
    *prev_margin = 0.0;
    cursor.y += gap;

    let p_t = resolve_length(&node.style.padding_top, containing.width);
    let p_r = resolve_length(&node.style.padding_right, containing.width);
    let p_b = resolve_length(&node.style.padding_bottom, containing.width);
    let p_l = resolve_length(&node.style.padding_left, containing.width);

    let w = resolve_length(&node.style.width, containing.width);
    let h = resolve_length(&node.style.height, containing.width);

    let avail = containing.width;
    if node.style.margin_left == crate::parsing::Length::Auto && node.style.margin_right == crate::parsing::Length::Auto {
        if w > 0.0 && w < avail {
            let surplus = avail - w - p_l - p_r;
            if surplus > 0.0 {
                m_l = surplus / 2.0;
                m_r = surplus / 2.0;
            }
        }
    } else if node.style.margin_left == crate::parsing::Length::Auto {
        if w > 0.0 {
            m_l = (avail - w - p_l - p_r - m_r).max(0.0);
        }
    } else if node.style.margin_right == crate::parsing::Length::Auto {
        if w > 0.0 {
            m_r = (avail - w - p_l - p_r - m_l).max(0.0);
        }
    }

    let content_w = avail - m_l - m_r;
    let inner_w = if w > 0.0 { w - p_l - p_r } else { content_w - p_l - p_r };

    let x = containing.x + m_l;
    let y = cursor.y;

    let content_x = x + p_l;
    let content_y = y + p_t;

    let mut children: Vec<LayoutBox> = Vec::new();
    let mut positioned: Vec<LayoutBox> = Vec::new();
    let mut child_cursor = Vec2 { x: content_x, y: content_y };
    let mut inline_batch: Vec<InlineItem> = Vec::new();
    let content_rect = Rect { x: content_x, y: content_y, width: inner_w, height: 0.0 };

    let child_cb = if node.style.position != Position::Static {
        CbContext { rect: content_rect, is_positioned: true }
    } else {
        CbContext { rect: content_rect, is_positioned: false }
    };

    let mut child_prev_margin = 0.0f32;

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
            inline_batch.push(InlineItem::Text(child.text.clone(), child.style.clone()));
            continue;
        }

        match &child.style.display {
            Display::Block => {
                flush_inlines(&mut inline_batch, &mut children, &content_rect, &mut child_cursor, floats);
                let cb = layout_block(child, &content_rect, &child_cb, &mut child_cursor, floats, &mut child_prev_margin);
                children.push(cb);
            }
            Display::InlineBlock => {
                let mut ib_cursor = Vec2 { x: content_rect.x, y: child_cursor.y };
                let mut zero_margin = 0.0f32;
                let ib_box = layout_block(child, &content_rect, &child_cb, &mut ib_cursor, floats, &mut zero_margin);
                inline_batch.push(InlineItem::InlineBlock(ib_box));
            }
            _ => {
                let mut inline_collected = Vec::new();
                collect_inline(child, &mut inline_collected);
                for n in inline_collected {
                    inline_batch.push(InlineItem::Text(n.text.clone(), n.style.clone()));
                }
            }
        }
    }

    flush_inlines(&mut inline_batch, &mut children, &content_rect, &mut child_cursor, floats);

    let content_h = child_cursor.y - content_y;
    let box_h = if h > 0.0 { h } else { content_h + p_t + p_b };
    cursor.y = y + box_h;

    *prev_margin = m_b;

    let clip = if node.style.overflow_x != Overflow::Visible || node.style.overflow_y != Overflow::Visible {
        Some(Rect { x, y, width: content_w, height: box_h })
    } else {
        None
    };

    let mut rect = Rect { x, y, width: content_w, height: box_h };
    if node.style.position == Position::Relative {
        rect.x += resolve_length(&node.style.left, avail) - resolve_length(&node.style.right, avail);
        rect.y += resolve_length(&node.style.top, avail) - resolve_length(&node.style.bottom, avail);
    }

    let mut box_ = LayoutBox::new(
        node.tag.clone(),
        node.text.clone(),
        node.style.clone(),
        rect,
    );
    box_.children = children;
    box_.positioned_children = positioned;
    box_.clip_rect = clip;
    box_.src = node.src.clone();
    box_
}

// --- Inline layout ---

#[derive(Debug)]
enum InlineItem {
    Text(String, ComputedValues),
    InlineBlock(LayoutBox),
}

fn collect_inline<'a>(node: &'a RenderNode, out: &mut Vec<&'a RenderNode>) {
    if node.tag == "#text" {
        out.push(node);
    } else {
        for child in &node.children {
            collect_inline(child, out);
        }
    }
}

fn split_words(text: &str, style: &ComputedValues) -> Vec<(String, f32)> {
    let space_w = estimate_text_width(" ", style.font_size);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        // whitespace-only node → single space token
        return vec![(String::from(" "), space_w)];
    }
    let mut out = Vec::with_capacity(words.len() * 2 - 1);
    for (i, w) in words.iter().enumerate() {
        if i > 0 || text.starts_with(' ') || text.starts_with('\t') || text.starts_with('\n') {
            out.push((String::from(" "), space_w));
        }
        let w_w = estimate_text_width(w, style.font_size);
        out.push((w.to_string(), w_w));
    }
    if text.ends_with(' ') || text.ends_with('\t') || text.ends_with('\n') {
        out.push((String::from(" "), space_w));
    }
    out
}

fn layout_inlines(items: Vec<InlineItem>, containing: &Rect, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) -> Vec<LayoutBox> {
    if items.is_empty() {
        return vec![];
    }

    let mut result = Vec::new();
    let mut line_items: Vec<(InlineItem, f32)> = Vec::new();
    let mut y = cursor.y;

    for item in items {
        match item {
            InlineItem::Text(text, style) => {
                let words = split_words(&text, &style);
                for (word_text, word_w) in &words {
                    let line_w: f32 = line_items.iter().map(|(_, iw)| iw).sum();
                    let avail = available_inline_width(containing.x, containing.x + containing.width, y, floats);

                    // If this word doesn't fit and we already have content on the line, wrap
                    if line_w > 0.0 && line_w + word_w > avail {
                        y = flush_line(&mut result, &mut line_items, containing.x, y);
                    }

                    // Don't start a line with a space
                    if line_items.is_empty() && word_text == " " {
                        continue;
                    }

                    line_items.push((InlineItem::Text(word_text.clone(), style.clone()), *word_w));
                }
            }
            InlineItem::InlineBlock(b) => {
                let w = b.rect.width;
                let line_w: f32 = line_items.iter().map(|(_, iw)| iw).sum();
                let avail = available_inline_width(containing.x, containing.x + containing.width, y, floats);

                if line_w > 0.0 && line_w + w > avail {
                    y = flush_line(&mut result, &mut line_items, containing.x, y);
                }

                line_items.push((InlineItem::InlineBlock(b), w));
            }
        }
    }

    if !line_items.is_empty() {
        flush_line(&mut result, &mut line_items, containing.x, y);
    }

    cursor.y = y;
    result
}

fn flush_line(result: &mut Vec<LayoutBox>, line: &mut Vec<(InlineItem, f32)>, origin_x: f32, y: f32) -> f32 {
    if line.is_empty() { return y; }

    // First pass: compute baseline offsets
    let mut max_above = 0.0f32;
    let mut max_below = 0.0f32;
    let mut boxes: Vec<LayoutBox> = Vec::with_capacity(line.len());
    let mut baseline_offsets: Vec<f32> = Vec::with_capacity(line.len());

    for (item, _) in line.iter() {
        match item {
            InlineItem::Text(text, style) => {
                let text_w = estimate_text_width(text, style.font_size);
                let box_h = style.font_size * 1.2;
                let above = box_h * 0.8;
                boxes.push(LayoutBox::new("#text".into(), text.clone(), style.clone(), Rect { x: 0.0, y: 0.0, width: text_w, height: box_h }));
                baseline_offsets.push(above);
                max_above = max_above.max(above);
                max_below = max_below.max(box_h - above);
            }
            InlineItem::InlineBlock(b) => {
                let above = b.rect.height;
                boxes.push(b.clone());
                baseline_offsets.push(above);
                max_above = max_above.max(above);
                // inline-block baseline at bottom
            }
        }
    }

    let line_height = (max_above + max_below).max(10.0);
    let baseline_y = y + max_above;

    // Second pass: position and output
    let mut x = origin_x;
    for (_i, (bx, bo)) in boxes.iter_mut().zip(baseline_offsets.iter()).enumerate() {
        bx.rect.x = x;
        bx.rect.y = baseline_y - bo;
        x += bx.rect.width;
        result.push(std::mem::replace(bx, LayoutBox::new(String::new(), String::new(), ComputedValues::default(), Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 })));
    }

    line.clear();
    y + line_height
}

fn flush_inlines(batch: &mut Vec<InlineItem>, children: &mut Vec<LayoutBox>, content_rect: &Rect, cursor: &mut Vec2, floats: &mut Vec<FloatBox>) {
    if !batch.is_empty() {
        let items: Vec<InlineItem> = batch.drain(..).collect();
        let lbs = layout_inlines(items, content_rect, cursor, floats);
        children.extend(lbs);
    }
}

fn available_inline_width(container_left: f32, container_right: f32, y: f32, floats: &[FloatBox]) -> f32 {
    let mut left = container_left;
    let mut right = container_right;
    for f in floats {
        if y >= f.rect.y && y < f.rect.y + f.rect.height {
            match f.float {
                Float::Left => left = left.max(f.rect.x + f.rect.width),
                Float::Right => right = right.min(f.rect.x),
                _ => {}
            }
        }
    }
    (right - left).max(0.0)
}

// --- Font ---

#[cfg(freetype_avail)]
fn font_cache() -> Option<&'static FontHandle> {
    static FONT: OnceLock<Option<FontHandle>> = OnceLock::new();
    FONT.get_or_init(|| {
        let paths = [
            crate::font::DEFAULT_FONT_PATH,
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

pub fn resolve_length(length: &Length, parent_width: f32) -> f32 {
    match length {
        Length::Px(v) => *v,
        Length::Percent(p) => parent_width * p / 100.0,
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

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::*;

    fn make_render(tag: &str, style: ComputedValues, children: Vec<RenderNode>) -> RenderNode {
        RenderNode { tag: tag.into(), text: String::new(), style, children, src: None }
    }

    fn make_text(text: &str, style: ComputedValues) -> RenderNode {
        RenderNode { tag: "#text".into(), text: text.into(), style, children: vec![], src: None }
    }

    fn base_style() -> ComputedValues {
        ComputedValues::default()
    }

    fn size(w: f32, h: f32) -> Size {
        Size { width: w, height: h }
    }

    #[test]
    fn test_block_layout_basic() {
        let root = make_render("div", base_style(), vec![
            make_render("p", base_style(), vec![
                make_text("hello", base_style()),
            ]),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].tag, "div");
        assert_eq!(boxes[0].rect.x, 0.0);
        assert_eq!(boxes[0].rect.y, 0.0);
        assert_eq!(boxes[0].rect.width, 800.0);
    }

    #[test]
    fn test_position_absolute() {
        let mut abs_style = base_style();
        abs_style.position = Position::Absolute;
        abs_style.top = Length::Px(10.0);
        abs_style.left = Length::Px(20.0);
        abs_style.width = Length::Px(100.0);
        abs_style.height = Length::Px(50.0);

        let root = make_render("div", base_style(), vec![
            make_render("abs", abs_style, vec![]),
            make_render("normal", base_style(), vec![]),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes.len(), 1);
        assert_eq!(boxes[0].children.len(), 1);
        // only normal child in normal flow, abs is positioned
        assert_eq!(boxes[0].children[0].tag, "normal");
    }

    #[test]
    fn test_float_left() {
        let mut float_style = base_style();
        float_style.float = Float::Left;
        float_style.width = Length::Px(200.0);
        float_style.height = Length::Px(100.0);

        let root = make_render("div", base_style(), vec![
            make_render("float", float_style, vec![]),
            make_render("normal", base_style(), vec![
                make_text("text after float", base_style()),
            ]),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes[0].children.len(), 2);
        // float should be first child, positioned at left
        assert_eq!(boxes[0].children[0].tag, "float");
        assert_eq!(boxes[0].children[0].rect.x, 0.0);
    }

    #[test]
    fn test_overflow_clip() {
        let mut clip_style = base_style();
        clip_style.overflow_x = Overflow::Hidden;
        clip_style.overflow_y = Overflow::Hidden;
        clip_style.width = Length::Px(100.0);
        clip_style.height = Length::Px(100.0);

        let root = make_render("clip", clip_style, vec![
            make_render("child", base_style(), vec![]),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes[0].tag, "clip");
        assert!(boxes[0].clip_rect.is_some());
    }

    #[test]
    fn test_margin() {
        let mut margin_style = base_style();
        margin_style.margin_top = Length::Px(10.0);
        margin_style.margin_left = Length::Px(20.0);

        let root = make_render("div", margin_style, vec![]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes[0].rect.x, 20.0);
        assert_eq!(boxes[0].rect.y, 10.0);
    }

    #[test]
    fn test_padding() {
        let mut pad_style = base_style();
        pad_style.padding_top = Length::Px(30.0);
        pad_style.padding_left = Length::Px(40.0);

        let root = make_render("div", pad_style, vec![]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes[0].rect.x, 0.0);
        assert_eq!(boxes[0].rect.y, 0.0);
    }

    #[test]
    fn test_inline_block() {
        let mut ib_style = base_style();
        ib_style.display = Display::InlineBlock;
        ib_style.width = Length::Px(100.0);
        ib_style.height = Length::Px(50.0);

        let root = make_render("div", base_style(), vec![
            make_text("before ", base_style()),
            make_render("span", ib_style, vec![make_text("inline-block", base_style())]),
            make_text(" after", base_style()),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes.len(), 1);
        let children = &boxes[0].children;
        // inline-block uses full width (auto), doesn't fit on line 1, sits on its own line
        // "before " → ["before", " "] (line 1)
        // span → own line (line 2, full width)
        // " after" → leading space skipped, "after" (line 3)
        assert_eq!(children.len(), 4);
        assert_eq!(children[0].text, "before");
        assert_eq!(children[1].text, " ");
        assert_eq!(children[2].tag, "span");
        assert_eq!(children[3].text, "after");
        assert_eq!(children[2].children.len(), 1);
        assert_eq!(children[2].children[0].text, "inline-block");
    }

    #[test]
    fn test_baseline_alignment() {
        let root = make_render("div", base_style(), vec![
            make_text("Hello World", base_style()),
        ]);
        let engine = BlockLayout;
        let boxes = engine.layout(&root, size(800.0, 600.0));
        assert_eq!(boxes.len(), 1);
        let children = &boxes[0].children;
        // "Hello", " ", "World" — word-level split
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].text, "Hello");
        assert_eq!(children[1].text, " ");
        assert_eq!(children[2].text, "World");
        // all on same line → same y
        assert_eq!(children[0].rect.y, children[1].rect.y);
        assert_eq!(children[1].rect.y, children[2].rect.y);
        // consecutive x positions
        assert!(children[0].rect.x < children[1].rect.x);
        assert!(children[1].rect.x < children[2].rect.x);
    }

    #[test]
    fn test_word_wrapping() {
        let root = make_render("div", base_style(), vec![
            make_text("one two three", base_style()),
        ]);
        let engine = BlockLayout;
        // Narrow — only one word fits per line
        let boxes = engine.layout(&root, size(50.0, 600.0));
        let children = &boxes[0].children;
        // Each word on its own line + trailing space = "one", " ", "two", " ", "three"
        assert_eq!(children.len(), 5);
        assert_eq!(children[0].text, "one");
        assert_eq!(children[1].text, " ");
        assert_eq!(children[2].text, "two");
        assert_eq!(children[3].text, " ");
        assert_eq!(children[4].text, "three");
        // Each word starts at x=0 (line start)
        assert_eq!(children[0].rect.x, 0.0);
        assert_eq!(children[2].rect.x, 0.0);
        assert_eq!(children[4].rect.x, 0.0);
        // Each subsequent word is on a new line
        assert!(children[2].rect.y > children[0].rect.y);
        assert!(children[4].rect.y > children[2].rect.y);
    }
}
