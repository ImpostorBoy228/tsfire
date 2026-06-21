use std::collections::HashMap;

use crate::ui_shit::layout::{LayoutBox, Rect, Size};
use crate::parsing::{Color, Display, BorderStyle as ParsedBorderStyle};

// --- Data types ---

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BorderStyle {
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
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

/// A decoded RGBA image for the display list.
#[derive(Clone, Debug)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

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
    DrawBoxShadow(Rect, Color, f32, f32, f32), // rect, color, offset_x, offset_y, blur
    SetClip(Rect),
    PopClip,
    SetOpacity(f32),
    PopOpacity,
}

/// Complete flat display list for a page.
/// No per-element heap allocations — text lives in `text_arena`.
#[derive(Debug)]
pub struct DisplayList {
    pub items: Vec<DisplayCommand>,
    pub text_arena: String,
    pub decoded_images: Vec<DecodedImage>,
    pub content_size: Size,
}

// --- Builder ---

/// Build a display list from layout boxes.
///
/// `image_map` maps CSS/element image URLs → index into `decoded_images`.
pub fn build_display_list(
    boxes: &[LayoutBox],
    decoded_images: Vec<DecodedImage>,
    image_map: &HashMap<String, u32>,
) -> DisplayList {
    let mut items = Vec::new();
    let mut text_arena = String::new();
    let content_size = box_tree_extent(boxes);

    for box_ in boxes {
        paint_box(box_, &mut items, &mut text_arena, image_map);
    }

    DisplayList { items, text_arena, decoded_images, content_size }
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

fn paint_box(box_: &LayoutBox, items: &mut Vec<DisplayCommand>, text_arena: &mut String, image_map: &HashMap<String, u32>) {
    if box_.style.opacity < 1.0 {
        items.push(DisplayCommand::SetOpacity(box_.style.opacity));
    }

    // 1. box-shadow (drawn behind background)
    for shadow in &box_.style.box_shadow {
        let sx = box_.rect.x + shadow.offset_x;
        let sy = box_.rect.y + shadow.offset_y;
        let sw = box_.rect.width;
        let sh = box_.rect.height;
        items.push(DisplayCommand::DrawBoxShadow(
            Rect { x: sx, y: sy, width: sw, height: sh },
            shadow.color, shadow.offset_x, shadow.offset_y, shadow.blur,
        ));
    }

    // 2. background fill / gradient / image
    if let Some(bi) = box_.style.background_image.first() {
        match bi {
            crate::parsing::BackgroundImage::Gradient { from, to, vertical } => {
                items.push(DisplayCommand::FillGradient(box_.rect, Gradient { from: *from, to: *to, vertical: *vertical }));
            }
            crate::parsing::BackgroundImage::Url(url) => {
                if let Some(&img_idx) = image_map.get(url) {
                    items.push(DisplayCommand::DrawImage(box_.rect, img_idx));
                }
            }
            _ => {}
        }
    } else {
        let bg = box_.style.background_color;
        if !is_transparent(&bg) && !is_white(&bg) {
            items.push(DisplayCommand::FillRect(box_.rect, bg));
        }
    }

    // 2. clip
    if let Some(cr) = box_.clip_rect {
        items.push(DisplayCommand::SetClip(cr));
    }

    // Paint-order sorting for children
    let (neg, zero, pos) = partition_by_z_index(&box_.children);

    // 3. z-index < 0 (ascending)
    for child in &neg {
        if child.style.display == Display::Block {
            paint_box(child, items, text_arena, image_map);
        }
    }

    // 4. z-index == 0 block then inline
    for child in &zero {
        if child.style.display == Display::Block {
            paint_box(child, items, text_arena, image_map);
        }
    }
    for child in &zero {
        if child.style.display != Display::Block {
            paint_box(child, items, text_arena, image_map);
        }
    }

    // 5. z-index > 0 (ascending)
    for child in &pos {
        paint_box(child, items, text_arena, image_map);
    }

    // 6. positioned children (painted above normal flow)
    let (neg_p, zero_p, pos_p) = partition_by_z_index(&box_.positioned_children);
    for c in &neg_p { paint_box(c, items, text_arena, image_map); }
    for c in &zero_p { paint_box(c, items, text_arena, image_map); }
    for c in &pos_p { paint_box(c, items, text_arena, image_map); }

    // 7. replaced elements (img)
    if box_.tag == "img" {
        if let Some(ref src) = box_.src {
            if let Some(&img_idx) = image_map.get(src) {
                items.push(DisplayCommand::DrawImage(box_.rect, img_idx));
            }
        }
    }

    // 8. text
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

    // 8. text-decoration (underline, overline, line-through)
    {
        let td = &box_.style.text_decoration_line;
        let tc = box_.style.text_decoration_color;
        if td.underline {
            let y = box_.rect.y + box_.rect.height - 1.0;
            items.push(DisplayCommand::FillRect(
                Rect { x: box_.rect.x, y, width: box_.rect.width, height: 1.0 },
                tc,
            ));
        }
        if td.overline {
            items.push(DisplayCommand::FillRect(
                Rect { x: box_.rect.x, y: box_.rect.y, width: box_.rect.width, height: 1.0 },
                tc,
            ));
        }
        if td.line_through {
            let y = box_.rect.y + box_.rect.height * 0.4;
            items.push(DisplayCommand::FillRect(
                Rect { x: box_.rect.x, y, width: box_.rect.width, height: 1.0 },
                tc,
            ));
        }
    }

    // 9. outline
    if box_.style.outline_width > 0.0 && box_.style.outline_style != ParsedBorderStyle::None {
        let ow = box_.style.outline_width;
        let oc = box_.style.outline_color;
        let r = box_.rect;
        // top
        items.push(DisplayCommand::FillRect(Rect { x: r.x - ow, y: r.y - ow, width: r.width + 2.0 * ow, height: ow }, oc));
        // bottom
        items.push(DisplayCommand::FillRect(Rect { x: r.x - ow, y: r.y + r.height, width: r.width + 2.0 * ow, height: ow }, oc));
        // left
        items.push(DisplayCommand::FillRect(Rect { x: r.x - ow, y: r.y, width: ow, height: r.height }, oc));
        // right
        items.push(DisplayCommand::FillRect(Rect { x: r.x + r.width, y: r.y, width: ow, height: r.height }, oc));
    }

    // 10. border
    let sides = extract_border(box_);
    if has_visible_border(&sides) {
        items.push(DisplayCommand::Border(box_.rect, sides));
    }

    if let Some(_) = box_.clip_rect {
        items.push(DisplayCommand::PopClip);
    }

    if box_.style.opacity < 1.0 {
        items.push(DisplayCommand::PopOpacity);
    }
}

// --- Helpers ---

fn is_transparent(c: &Color) -> bool { c.3 == 0 }
fn is_white(c: &Color) -> bool { c.0 == 255 && c.1 == 255 && c.2 == 255 && c.3 == 255 }

fn to_paint_border_style(s: ParsedBorderStyle) -> BorderStyle {
    match s {
        ParsedBorderStyle::None => BorderStyle::None,
        ParsedBorderStyle::Solid => BorderStyle::Solid,
        ParsedBorderStyle::Dashed => BorderStyle::Dashed,
        ParsedBorderStyle::Dotted => BorderStyle::Dotted,
        ParsedBorderStyle::Double => BorderStyle::Double,
        _ => BorderStyle::Solid,
    }
}

fn has_visible_border(sides: &[BorderSide; 4]) -> bool {
    sides.iter().any(|s| s.width > 0.0 && s.style != BorderStyle::None)
}

fn extract_border(box_: &LayoutBox) -> [BorderSide; 4] {
    let s = &box_.style;
    let bw = |w: f32| if w > 0.0 { w } else { 0.0 };
    [
        BorderSide { width: bw(s.border_top_width), color: s.border_top_color, style: to_paint_border_style(s.border_top_style) },
        BorderSide { width: bw(s.border_right_width), color: s.border_right_color, style: to_paint_border_style(s.border_right_style) },
        BorderSide { width: bw(s.border_bottom_width), color: s.border_bottom_color, style: to_paint_border_style(s.border_bottom_style) },
        BorderSide { width: bw(s.border_left_width), color: s.border_left_color, style: to_paint_border_style(s.border_left_style) },
    ]
}

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
                let end = (range.start + range.len) as usize;
                let text = if end <= list.text_arena.len() { &list.text_arena[range.start as usize..end] } else { "" };
                let preview: String = text.chars().take(40).collect();
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
            DisplayCommand::DrawBoxShadow(r, c, ox, oy, bl) =>
                println!("  {:4}. BoxShadow  ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) off({},{}) blur{} #{:02x}{:02x}{:02x}",
                    i, r.x, r.y, r.width, r.height, ox, oy, bl, c.0, c.1, c.2),
        }
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::{ComputedValues, Overflow};
    use crate::ui_shit::layout::LayoutBox;

    fn make_layout(tag: &str, style: ComputedValues, children: Vec<LayoutBox>) -> LayoutBox {
        LayoutBox {
            tag: tag.into(), text: String::new(), style, rect: Rect { x: 0.0, y: 0.0, width: 100.0, height: 50.0 },
            children, positioned_children: vec![], clip_rect: None, src: None,
        }
    }

    fn make_text_box(text: &str) -> LayoutBox {
        let mut s = ComputedValues::default();
        s.color = Color(0, 0, 0, 255);
        LayoutBox {
            tag: "#text".into(), text: text.into(), style: s,
            rect: Rect { x: 10.0, y: 10.0, width: 80.0, height: 20.0 },
            children: vec![], positioned_children: vec![], clip_rect: None, src: None,
        }
    }

    fn collect_names(list: &DisplayList) -> Vec<&'static str> {
        list.items.iter().map(|cmd| {
            match cmd {
                DisplayCommand::FillRect(..) => "FillRect",
                DisplayCommand::FillGradient(..) => "FillGradient",
                DisplayCommand::DrawImage(..) => "DrawImage",
                DisplayCommand::TextRun(..) => "TextRun",
                DisplayCommand::Border(..) => "Border",
                DisplayCommand::SetClip(..) => "SetClip",
                DisplayCommand::PopClip => "PopClip",
                DisplayCommand::SetOpacity(..) => "SetOpacity",
                DisplayCommand::PopOpacity => "PopOpacity",
                DisplayCommand::DrawBoxShadow(..) => "BoxShadow",
            }
        }).collect()
    }

    fn empty_image_map() -> HashMap<String, u32> { HashMap::new() }

    #[test]
    fn test_empty_display_list() {
        let dl = build_display_list(&[], vec![], &empty_image_map());
        assert!(dl.items.is_empty());
    }

    #[test]
    fn test_text_run() {
        let box_ = make_text_box("hello");
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        assert_eq!(collect_names(&dl), vec!["TextRun"]);
    }

    #[test]
    fn test_border_generated() {
        let mut s = ComputedValues::default();
        s.border_top_width = 2.0;
        s.border_top_style = ParsedBorderStyle::Solid;
        s.border_top_color = Color(255, 0, 0, 255);
        s.border_right_width = 2.0;
        s.border_right_style = ParsedBorderStyle::Solid;
        s.border_bottom_width = 2.0;
        s.border_bottom_style = ParsedBorderStyle::Solid;
        s.border_left_width = 2.0;
        s.border_left_style = ParsedBorderStyle::Solid;

        let box_ = make_layout("div", s, vec![]);
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(names.contains(&"Border"), "expected Border command, got {:?}", names);
    }

    #[test]
    fn test_no_border_when_zero_width() {
        let s = ComputedValues::default();
        let box_ = make_layout("div", s, vec![]);
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(!names.contains(&"Border"));
    }

    #[test]
    fn test_opacity_wrapping() {
        let mut s = ComputedValues::default();
        s.opacity = 0.5;
        let child = make_text_box("inner");
        let box_ = make_layout("div", s, vec![child]);
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(names.contains(&"SetOpacity"), "expected SetOpacity, got {:?}", names);
        assert!(names.contains(&"PopOpacity"), "expected PopOpacity, got {:?}", names);
        let pos_set = names.iter().position(|&n| n == "SetOpacity").unwrap();
        let pos_pop = names.iter().position(|&n| n == "PopOpacity").unwrap();
        assert!(pos_set < pos_pop, "SetOpacity should come before PopOpacity");
    }

    #[test]
    fn test_clip_wrapping() {
        let mut s = ComputedValues::default();
        s.overflow_x = Overflow::Hidden;
        s.overflow_y = Overflow::Hidden;
        let mut box_ = make_layout("div", s, vec![]);
        box_.clip_rect = Some(Rect { x: 0.0, y: 0.0, width: 100.0, height: 50.0 });
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(names.contains(&"SetClip"), "expected SetClip, got {:?}", names);
        assert!(names.contains(&"PopClip"), "expected PopClip, got {:?}", names);
    }

    #[test]
    fn test_background_gradient() {
        let mut s = ComputedValues::default();
        s.background_image = vec![crate::parsing::BackgroundImage::Gradient {
            from: Color(255, 0, 0, 255),
            to: Color(0, 0, 255, 255),
            vertical: true,
        }];
        let box_ = make_layout("div", s, vec![]);
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(names.contains(&"FillGradient"), "expected FillGradient, got {:?}", names);
    }

    #[test]
    fn test_children_are_painted() {
        let child = make_text_box("child");
        let box_ = make_layout("div", ComputedValues::default(), vec![child]);
        let dl = build_display_list(&[box_], vec![], &empty_image_map());
        let names = collect_names(&dl);
        assert!(names.contains(&"TextRun"));
    }
}
