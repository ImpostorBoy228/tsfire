use crate::layout::{LayoutBox, Rect, Size};
use crate::style::{Color, Display};

/// A single drawing command.
/// Flat list, ordered by CSS paint order (back-to-front).
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// Solid colour fill (background, border, etc.)
    SolidColor {
        rect: Rect,
        color: Color,
    },
    /// Text fragment.
    /// `rect` contains the exact content area (x, y, width, height).
    Text {
        rect: Rect,
        text: String,
        color: Color,
        font_size: f32,
    },
    /// Border rectangle — drawn on top of background, inside padding edge.
    BorderBox {
        rect: Rect,
        color: Color,
        width: f32,
    },
}

/// The complete flat display list for the page.
#[derive(Debug)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    pub content_size: Size,
}

/// Build a display list from the layout tree.
///
/// Paint order (per CSS 2.2 Appendix E):
///   1. background of current box
///   2. children with z-index < 0  (ascending)
///   3. block-level children with z-index == 0
///   4. inline  children with z-index == 0
///   5. children with z-index > 0  (ascending)
///   6. border of current box
pub fn build_display_list(boxes: &[LayoutBox]) -> DisplayList {
    let mut items = Vec::new();
    let mut content_w = 0.0_f32;
    let mut content_h = 0.0_f32;

    for box_ in boxes {
        paint_box(box_, &mut items);
        // track max extent for content_size
        let r = box_.rect;
        let far_x = r.x + r.width;
        let far_y = r.y + r.height;
        if far_x > content_w {
            content_w = far_x;
        }
        if far_y > content_h {
            content_h = far_y;
        }
    }

    DisplayList {
        items,
        content_size: Size { width: content_w, height: content_h },
    }
}

fn paint_box(box_: &LayoutBox, items: &mut Vec<DisplayItem>) {
    // 1. background
    let bg = box_.style.background_color;
    if !is_transparent(&bg) && !is_white(&bg) {
        items.push(DisplayItem::SolidColor {
            rect: box_.rect,
            color: bg,
        });
    }

    // Partition children for paint-order sorting
    let (neg, zero, pos) = partition_by_z_index(&box_.children);

    // 2. z-index < 0  (ascending – smaller (more negative) first)
    for child in &neg {
        paint_box(child, items);
    }

    // 3. block-level + 4. inline children with z-index == 0
    for child in &zero {
        if is_block_level(child) {
            paint_box(child, items);
        }
    }
    for child in &zero {
        if !is_block_level(child) {
            paint_box(child, items);
        }
    }

    // 5. z-index > 0 (ascending)
    for child in &pos {
        paint_box(child, items);
    }

    // 6. text (leaf nodes)
    if box_.tag == "#text" && !box_.text.is_empty() {
        items.push(DisplayItem::Text {
            rect: box_.rect,
            text: box_.text.clone(),
            color: box_.style.color,
            font_size: box_.style.font_size,
        });
    }

    // 7. border
    let border_w = detect_border_width(box_);
    if border_w > 0.0 {
        let border_color = detect_border_color(box_);
        items.push(DisplayItem::BorderBox {
            rect: box_.rect,
            color: border_color,
            width: border_w,
        });
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn is_transparent(c: &Color) -> bool {
    c.3 == 0
}

fn is_white(c: &Color) -> bool {
    c.0 == 255 && c.1 == 255 && c.2 == 255 && c.3 == 255
}

fn is_block_level(box_: &LayoutBox) -> bool {
    matches!(box_.style.display, Display::Block)
}

fn partition_by_z_index(children: &[LayoutBox]) -> (Vec<&LayoutBox>, Vec<&LayoutBox>, Vec<&LayoutBox>) {
    let mut neg = Vec::new();
    let mut zero = Vec::new();
    let mut pos = Vec::new();

    for child in children {
        let z = child.style.z_index;
        if z < 0 {
            neg.push(child);
        } else if z == 0 {
            zero.push(child);
        } else {
            pos.push(child);
        }
    }

    // sort by z-index ascending
    neg.sort_by_key(|b| b.style.z_index);
    pos.sort_by_key(|b| b.style.z_index);

    (neg, zero, pos)
}

/// Crude border detection — checks if the element has a non-zero computed border
/// (we parse `border-width` properties but they default to 0).
fn detect_border_width(_box_: &LayoutBox) -> f32 {
    // TODO: parse border-*-width into ComputedValues
    0.0
}

fn detect_border_color(_box_: &LayoutBox) -> Color {
    Color(0, 0, 0, 255)
}

// ---------------------------------------------------------------------------
// dump
// ---------------------------------------------------------------------------

pub fn dump_display_list(list: &DisplayList) {
    println!("--- display list ({} items, content {}x{}) ---",
        list.items.len(), list.content_size.width as u32, list.content_size.height as u32);
    for (i, item) in list.items.iter().enumerate() {
        match item {
            DisplayItem::SolidColor { rect, color } => {
                println!("  {:4}. SolidColor  ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) #{:02x}{:02x}{:02x}",
                    i, rect.x, rect.y, rect.width, rect.height, color.0, color.1, color.2);
            }
            DisplayItem::Text { rect, text, color, font_size } => {
                let preview: String = text.chars().take(40).collect();
                println!("  {:4}. Text       ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) fz{} color:#{:02x}{:02x}{:02x} '{}'",
                    i, rect.x, rect.y, rect.width, rect.height,
                    *font_size as u32, color.0, color.1, color.2, preview);
            }
            DisplayItem::BorderBox { rect, color, width } => {
                println!("  {:4}. Border     ({:>8.1},{:>4.1} {:>6.1}x{:<4.1}) w{} #{:02x}{:02x}{:02x}",
                    i, rect.x, rect.y, rect.width, rect.height,
                    *width as u32, color.0, color.1, color.2);
            }
        }
    }
}
