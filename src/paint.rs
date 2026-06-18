use crate::layout::{LayoutBox, Rect, Size};
use crate::style::{Color, Display};

// --- Data types ---

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Copy)]
pub struct BorderSide {
    pub width: f32,
    pub color: Color,
    pub style: BorderStyle,
}

#[derive(Debug, Clone, Copy)]
pub struct Gradient {
    pub from: Color,
    pub to: Color,
    pub vertical: bool,
}

pub type ImageIndex = u32;
pub type FontFamily = u8;

#[derive(Debug, Clone, Copy)]
pub struct TextRange {
    pub start: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub data_offset: u64,
    pub data_len: u64,
}

/// Flat drawing command, ~32-48 bytes each.
#[derive(Debug, Clone)]
pub enum DisplayCommand {
    FillRect(Rect, Color),
    FillGradient(Rect, Gradient),
    DrawImage(Rect, ImageIndex),
    TextRun(Rect, Color, f32, FontFamily, TextRange),
    Border(Rect, [BorderSide; 4]),
    SetClip(Rect),
    PopClip,
    SetOpacity(f32),
    PopOpacity,
}

/// Complete flat display list for a page.
/// No per-element heap allocations — text lives in `text_arena`, images in `images`.
#[derive(Debug)]
pub struct DisplayList {
    pub items: Vec<DisplayCommand>,
    pub text_arena: String,
    pub images: Vec<ImageData>,
    pub content_size: Size,
}

// --- Builder ---

pub fn build_display_list(boxes: &[LayoutBox]) -> DisplayList {
    let mut items = Vec::new();
    let mut text_arena = String::new();
    let content_size = box_tree_extent(boxes);

    for box_ in boxes {
        paint_box(box_, &mut items, &mut text_arena);
    }

    DisplayList { items, text_arena, images: Vec::new(), content_size }
}

fn box_tree_extent(boxes: &[LayoutBox]) -> Size {
    let mut w = 0.0;
    let mut h = 0.0;
    for b in boxes {
        let r = b.rect;
        if r.x + r.width > w { w = r.x + r.width; }
        if r.y + r.height > h { h = r.y + r.height; }
    }
    Size { width: w, height: h }
}

fn paint_box(box_: &LayoutBox, items: &mut Vec<DisplayCommand>, text_arena: &mut String) {
    // 1. background
    let bg = box_.style.background_color;
    if !is_transparent(&bg) && !is_white(&bg) {
        items.push(DisplayCommand::FillRect(box_.rect, bg));
    }

    // Paint-order sorting for children
    let (neg, zero, pos) = partition_by_z_index(&box_.children);

    // 2. z-index < 0 (ascending)
    for child in &neg {
        if child.style.display == Display::Block {
            paint_box(child, items, text_arena);
        }
    }

    // 3. z-index == 0 block then inline
    for child in &zero {
        if child.style.display == Display::Block {
            paint_box(child, items, text_arena);
        }
    }
    for child in &zero {
        if child.style.display != Display::Block {
            paint_box(child, items, text_arena);
        }
    }

    // 4. z-index > 0 (ascending)
    for child in &pos {
        paint_box(child, items, text_arena);
    }

    // 5. text
    if box_.tag == "#text" && !box_.text.is_empty() {
        let start = text_arena.len() as u32;
        text_arena.push_str(&box_.text);
        items.push(DisplayCommand::TextRun(
            box_.rect,
            box_.style.color,
            box_.style.font_size,
            0,  // default font family
            TextRange { start, len: box_.text.len() as u32 },
        ));
    }

    // 6. border
    let bw = detect_border_width(box_);
    if bw > 0.0 {
        let bc = detect_border_color(box_);
        let side = BorderSide { width: bw, color: bc, style: BorderStyle::Solid };
        items.push(DisplayCommand::Border(box_.rect, [side; 4]));
    }
}

// --- Helpers ---

fn is_transparent(c: &Color) -> bool { c.3 == 0 }
fn is_white(c: &Color) -> bool { c.0 == 255 && c.1 == 255 && c.2 == 255 && c.3 == 255 }

fn partition_by_z_index(children: &[LayoutBox]) -> (Vec<&LayoutBox>, Vec<&LayoutBox>, Vec<&LayoutBox>) {
    let mut neg = Vec::new();
    let mut zero = Vec::new();
    let mut pos = Vec::new();
    for child in children {
        let z = child.style.z_index;
        if z < 0 { neg.push(child); }
        else if z == 0 { zero.push(child); }
        else { pos.push(child); }
    }
    neg.sort_by_key(|b| b.style.z_index);
    pos.sort_by_key(|b| b.style.z_index);
    (neg, zero, pos)
}

fn detect_border_width(_box_: &LayoutBox) -> f32 { 0.0 }
fn detect_border_color(_box_: &LayoutBox) -> Color { Color(0, 0, 0, 255) }

// --- Dump ---

pub fn dump_display_list(list: &DisplayList) {
    println!("--- display list ({} items, text_arena={}B, content {}x{}) ---",
        list.items.len(), list.text_arena.len(),
        list.content_size.width as u32, list.content_size.height as u32);
    for (i, cmd) in list.items.iter().enumerate() {
        match cmd {
            DisplayCommand::FillRect(r, c) =>
                println!("  {:4}. FillRect   ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) #{:02x}{:02x}{:02x}",
                    i, r.x, r.y, r.width, r.height, c.0, c.1, c.2),
            DisplayCommand::FillGradient(r, g) =>
                println!("  {:4}. FillGradient ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) {:?}→{:?}",
                    i, r.x, r.y, r.width, r.height, g.from, g.to),
            DisplayCommand::DrawImage(r, idx) =>
                println!("  {:4}. DrawImage  ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) img#{}",
                    i, r.x, r.y, r.width, r.height, idx),
            DisplayCommand::TextRun(r, c, fz, _ff, range) => {
                let preview: String = list.text_arena[range.start as usize..][..range.len as usize].chars().take(40).collect();
                println!("  {:4}. TextRun    ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) fz{} color:#{:02x}{:02x}{:02x} '{}'",
                    i, r.x, r.y, r.width, r.height, *fz as u32, c.0, c.1, c.2, preview);
            }
            DisplayCommand::Border(r, _sides) =>
                println!("  {:4}. Border     ({:>8.1},{:>4.1} {:>6.1}x{:<4.1})",
                    i, r.x, r.y, r.width, r.height),
            DisplayCommand::SetClip(r) =>
                println!("  {:4}. SetClip    ({:>8.1},{:>4.1} {:>6.1}x{:<4.1})", i, r.x, r.y, r.width, r.height),
            DisplayCommand::PopClip =>
                println!("  {:4}. PopClip", i),
            DisplayCommand::SetOpacity(v) =>
                println!("  {:4}. SetOpacity  {}", i, v),
            DisplayCommand::PopOpacity =>
                println!("  {:4}. PopOpacity", i),
        }
    }
}
