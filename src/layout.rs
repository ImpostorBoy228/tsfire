use crate::style::{ComputedValues, Display, Length};
use crate::render::RenderNode;

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

// --- Layout box ---

#[derive(Debug)]
pub struct LayoutBox {
    pub tag: String,
    pub text: String,
    pub style: ComputedValues,
    pub rect: Rect,
    pub children: Vec<LayoutBox>,
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
        layout_children(root, &Rect { x: 0.0, y: 0.0, width: viewport.width, height: viewport.height }, &mut cursor, &mut boxes);
        boxes
    }
}

fn layout_children(node: &RenderNode, containing: &Rect, cursor: &mut Vec2, out: &mut Vec<LayoutBox>) {
    let display = &node.style.display;

    if *display == Display::None {
        return;
    }

    match display {
        Display::Block => {
            let box_ = layout_block(node, containing, cursor);
            out.push(box_);
        }
        Display::Inline | _ => {
            let mut inline_children = Vec::new();
            collect_inline(node, &mut inline_children);
            let mut boxes = layout_inlines(&inline_children, containing, cursor);
            out.append(&mut boxes);
        }
    }
}

fn layout_block(node: &RenderNode, containing: &Rect, cursor: &mut Vec2) -> LayoutBox {
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

    let x = containing.x + m_l;
    let y = cursor.y + m_t;

    let content_w = containing.width - m_l - m_r;
    let inner_w = if w > 0.0 { w } else { content_w - p_l - p_r };

    let content_x = x + p_l;
    let content_y = y + p_t;

    let mut children: Vec<LayoutBox> = Vec::new();
    let mut child_cursor = Vec2 { x: content_x, y: content_y };

    let mut inline_batch: Vec<&RenderNode> = Vec::new();

    fn flush_inline(inline_batch: &mut Vec<&RenderNode>, children: &mut Vec<LayoutBox>,
                    containing: &Rect, cursor: &mut Vec2) {
        if !inline_batch.is_empty() {
            let mut lbs = layout_inlines(inline_batch, containing, cursor);
            children.append(&mut lbs);
            inline_batch.clear();
        }
    }

    for child in &node.children {
        if child.tag == "#text" {
            inline_batch.push(child);
            continue;
        }

        match &child.style.display {
            Display::Block => {
                flush_inline(&mut inline_batch, &mut children,
                    &Rect { x: content_x, y: child_cursor.y, width: inner_w, height: 0.0 },
                    &mut child_cursor);
                let cb = layout_block(child,
                    &Rect { x: content_x, y: child_cursor.y, width: inner_w, height: 0.0 },
                    &mut child_cursor);
                children.push(cb);
            }
            _ => {
                let mut inline_collected = Vec::new();
                collect_inline(child, &mut inline_collected);
                inline_batch.extend(inline_collected);
            }
        }
    }

    flush_inline(&mut inline_batch, &mut children,
        &Rect { x: content_x, y: child_cursor.y, width: inner_w, height: 0.0 },
        &mut child_cursor);

    let content_h = child_cursor.y - content_y;
    let box_h = if h > 0.0 { h } else { content_h + p_t + p_b };
    cursor.y = y + box_h + m_b;

    LayoutBox {
        tag: node.tag.clone(),
        text: node.text.clone(),
        style: node.style.clone(),
        rect: Rect { x, y, width: content_w, height: box_h },
        children,
    }
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

fn layout_inlines(nodes: &[&RenderNode], containing: &Rect, cursor: &mut Vec2) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    let mut line_x = containing.x;

    for node in nodes {
        if node.tag == "#text" {
            let text_w = estimate_text_width(&node.text, node.style.font_size);
            if line_x + text_w > containing.x + containing.width && line_x > containing.x {
                cursor.y += node.style.font_size * 1.2;
                line_x = containing.x;
            }
            boxes.push(LayoutBox {
                tag: "#text".into(),
                text: node.text.clone(),
                style: node.style.clone(),
                rect: Rect { x: line_x, y: cursor.y, width: text_w, height: node.style.font_size * 1.2 },
                children: vec![],
            });
            line_x += text_w;
        }
    }

    if !nodes.is_empty() {
        cursor.y += nodes.last().unwrap().style.font_size * 1.2;
    }

    boxes
}

fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let char_w = font_size * 0.6;
    text.len() as f32 * char_w
}

fn resolve_length(length: &Length, _parent_width: f32) -> f32 {
    match length {
        Length::Px(v) => *v,
        Length::Auto => 0.0,
    }
}

// --- Layout tree dump ---

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
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
        println!("{}<{}> {:>8.1},{:>4.1} {:>6.1}x{:<4.1}",
            indent, box_.tag, box_.rect.x, box_.rect.y, box_.rect.width, box_.rect.height);
        for child in &box_.children {
            dump_box(child, depth + 1);
        }
    }
}

pub fn dump_boxes(boxes: &[LayoutBox]) {
    for box_ in boxes {
        dump_box(box_, 0);
    }
}
