use super::parse::{CssRule, DomElement, rule_matches_element};

// --- CSS value types ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Auto,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Display {
    Inline,
    Block,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Position {
    Static,
    Relative,
    Absolute,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BorderStyle {
    None,
    Hidden,
    Solid,
    Dashed,
    Dotted,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
    Clip,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Float {
    None,
    Left,
    Right,
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clear {
    None,
    Left,
    Right,
    Both,
    InlineStart,
    InlineEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Justify,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalAlign {
    Baseline,
    Top,
    Bottom,
    Middle,
    TextTop,
    TextBottom,
    Sub,
    Super,
    Length(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextDecorationLine {
    pub underline: bool,
    pub overline: bool,
    pub line_through: bool,
    pub blink: bool,
}

impl TextDecorationLine {
    pub fn none() -> Self {
        TextDecorationLine { underline: false, overline: false, line_through: false, blink: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BackgroundImage {
    None,
    Gradient { from: Color, to: Color, vertical: bool },
    Url(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

// --- Computed values for a styled element ---

#[derive(Clone, Debug)]
pub struct ComputedValues {
    pub display: Display,
    pub position: Position,
    pub width: Length,
    pub height: Length,
    pub margin_top: Length,
    pub margin_right: Length,
    pub margin_bottom: Length,
    pub margin_left: Length,
    pub padding_top: Length,
    pub padding_right: Length,
    pub padding_bottom: Length,
    pub padding_left: Length,
    pub color: Color,
    pub background_color: Color,
    pub font_size: f32,
    pub z_index: i32,

    // Border
    pub border_top_width: f32,
    pub border_right_width: f32,
    pub border_bottom_width: f32,
    pub border_left_width: f32,
    pub border_top_style: BorderStyle,
    pub border_right_style: BorderStyle,
    pub border_bottom_style: BorderStyle,
    pub border_left_style: BorderStyle,
    pub border_top_color: Color,
    pub border_right_color: Color,
    pub border_bottom_color: Color,
    pub border_left_color: Color,
    pub border_top_left_radius: f32,
    pub border_top_right_radius: f32,
    pub border_bottom_right_radius: f32,
    pub border_bottom_left_radius: f32,

    // Effects
    pub opacity: f32,
    pub box_shadow: Vec<BoxShadow>,

    // Layout
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub float: Float,
    pub clear: Clear,
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,

    // Background
    pub background_image: Vec<BackgroundImage>,

    // Text
    pub text_decoration_line: TextDecorationLine,
    pub text_decoration_color: Color,
    pub text_decoration_style: BorderStyle,
    pub text_align: TextAlign,
    pub vertical_align: VerticalAlign,

    // Outline
    pub outline_width: f32,
    pub outline_style: BorderStyle,
    pub outline_color: Color,
}

impl Default for ComputedValues {
    fn default() -> Self {
        ComputedValues {
            display: Display::Block,
            position: Position::Static,
            width: Length::Auto,
            height: Length::Auto,
            margin_top: Length::Px(0.0),
            margin_right: Length::Px(0.0),
            margin_bottom: Length::Px(0.0),
            margin_left: Length::Px(0.0),
            padding_top: Length::Px(0.0),
            padding_right: Length::Px(0.0),
            padding_bottom: Length::Px(0.0),
            padding_left: Length::Px(0.0),
            color: Color(0, 0, 0, 255),
            background_color: Color(255, 255, 255, 255),
            font_size: 16.0,
            z_index: 0,

            border_top_width: 0.0,
            border_right_width: 0.0,
            border_bottom_width: 0.0,
            border_left_width: 0.0,
            border_top_style: BorderStyle::None,
            border_right_style: BorderStyle::None,
            border_bottom_style: BorderStyle::None,
            border_left_style: BorderStyle::None,
            border_top_color: Color(0, 0, 0, 255),
            border_right_color: Color(0, 0, 0, 255),
            border_bottom_color: Color(0, 0, 0, 255),
            border_left_color: Color(0, 0, 0, 255),
            border_top_left_radius: 0.0,
            border_top_right_radius: 0.0,
            border_bottom_right_radius: 0.0,
            border_bottom_left_radius: 0.0,

            opacity: 1.0,
            box_shadow: vec![],

            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            float: Float::None,
            clear: Clear::None,
            top: Length::Auto,
            right: Length::Auto,
            bottom: Length::Auto,
            left: Length::Auto,

            background_image: vec![],

            text_decoration_line: TextDecorationLine::none(),
            text_decoration_color: Color(0, 0, 0, 255),
            text_decoration_style: BorderStyle::Solid,
            text_align: TextAlign::Start,
            vertical_align: VerticalAlign::Baseline,

            outline_width: 0.0,
            outline_style: BorderStyle::None,
            outline_color: Color(0, 0, 0, 255),
        }
    }
}

impl ComputedValues {
    pub fn inherit(parent: &ComputedValues) -> Self {
        let mut cv = ComputedValues::default();
        cv.color = parent.color.clone();
        cv.font_size = parent.font_size;
        cv.text_align = parent.text_align;
        cv
    }

    pub fn apply(&mut self, property: &str, value: &str) {
        match property {
            "display" => {
                if let Some(v) = parse_display(value) { self.display = v; }
            }
            "position" => {
                if let Some(v) = parse_position(value) { self.position = v; }
            }
            "width" => {
                if let Some(v) = parse_length(value) { self.width = v; }
            }
            "height" => {
                if let Some(v) = parse_length(value) { self.height = v; }
            }
            "margin-top" => {
                if let Some(v) = parse_length(value) { self.margin_top = v; }
            }
            "margin-right" => {
                if let Some(v) = parse_length(value) { self.margin_right = v; }
            }
            "margin-bottom" => {
                if let Some(v) = parse_length(value) { self.margin_bottom = v; }
            }
            "margin-left" => {
                if let Some(v) = parse_length(value) { self.margin_left = v; }
            }
            "margin" => {
                if let Some((t, r, b, l)) = parse_margin_shorthand(value) {
                    self.margin_top = t;
                    self.margin_right = r;
                    self.margin_bottom = b;
                    self.margin_left = l;
                }
            }
            "padding-top" => {
                if let Some(v) = parse_length(value) { self.padding_top = v; }
            }
            "padding-right" => {
                if let Some(v) = parse_length(value) { self.padding_right = v; }
            }
            "padding-bottom" => {
                if let Some(v) = parse_length(value) { self.padding_bottom = v; }
            }
            "padding-left" => {
                if let Some(v) = parse_length(value) { self.padding_left = v; }
            }
            "padding" => {
                if let Some((t, r, b, l)) = parse_margin_shorthand(value) {
                    self.padding_top = t;
                    self.padding_right = r;
                    self.padding_bottom = b;
                    self.padding_left = l;
                }
            }
            "color" => {
                if let Some(v) = parse_color(value) { self.color = v; }
            }
            "background-color" | "background" => {
                if let Some(v) = parse_color(value) { self.background_color = v; }
            }
            "font-size" => {
                if let Some(v) = parse_font_size(value) { self.font_size = v; }
            }
            "z-index" => {
                if let Ok(v) = value.trim().parse::<i32>() { self.z_index = v; }
            }

            "border-top-width" => {
                if let Some(v) = parse_border_width(value) { self.border_top_width = v; }
            }
            "border-right-width" => {
                if let Some(v) = parse_border_width(value) { self.border_right_width = v; }
            }
            "border-bottom-width" => {
                if let Some(v) = parse_border_width(value) { self.border_bottom_width = v; }
            }
            "border-left-width" => {
                if let Some(v) = parse_border_width(value) { self.border_left_width = v; }
            }
            "border-top-style" => {
                if let Some(v) = parse_border_style(value) { self.border_top_style = v; }
            }
            "border-right-style" => {
                if let Some(v) = parse_border_style(value) { self.border_right_style = v; }
            }
            "border-bottom-style" => {
                if let Some(v) = parse_border_style(value) { self.border_bottom_style = v; }
            }
            "border-left-style" => {
                if let Some(v) = parse_border_style(value) { self.border_left_style = v; }
            }
            "border-top-color" => {
                if let Some(v) = parse_color(value) { self.border_top_color = v; }
            }
            "border-right-color" => {
                if let Some(v) = parse_color(value) { self.border_right_color = v; }
            }
            "border-bottom-color" => {
                if let Some(v) = parse_color(value) { self.border_bottom_color = v; }
            }
            "border-left-color" => {
                if let Some(v) = parse_color(value) { self.border_left_color = v; }
            }
            "opacity" => {
                if let Ok(v) = value.trim().parse::<f32>() { self.opacity = v; }
            }
            "overflow-x" => {
                if let Some(v) = parse_overflow(value) { self.overflow_x = v; }
            }
            "overflow-y" => {
                if let Some(v) = parse_overflow(value) { self.overflow_y = v; }
            }
            "overflow" => {
                if let Some(v) = parse_overflow(value) { self.overflow_x = v; self.overflow_y = v; }
            }
            "float" => {
                if let Some(v) = parse_float(value) { self.float = v; }
            }
            "clear" => {
                if let Some(v) = parse_clear(value) { self.clear = v; }
            }
            "top" => {
                if let Some(v) = parse_length(value) { self.top = v; }
            }
            "right" => {
                if let Some(v) = parse_length(value) { self.right = v; }
            }
            "bottom" => {
                if let Some(v) = parse_length(value) { self.bottom = v; }
            }
            "left" => {
                if let Some(v) = parse_length(value) { self.left = v; }
            }
            "text-align" => {
                if let Some(v) = parse_text_align(value) { self.text_align = v; }
            }
            "vertical-align" => {
                if let Some(v) = parse_vertical_align(value) { self.vertical_align = v; }
            }
            "outline-width" => {
                if let Some(v) = parse_border_width(value) { self.outline_width = v; }
            }
            "outline-style" => {
                if let Some(v) = parse_border_style(value) { self.outline_style = v; }
            }
            "outline-color" => {
                if let Some(v) = parse_color(value) { self.outline_color = v; }
            }
            _ => {}
        }
    }
}

// --- Cascade: match rules → sort by specificity → merge ---

pub fn cascade(rules: &[CssRule], element: &DomElement) -> ComputedValues {
    let mut matched: Vec<(&CssRule, u32)> = Vec::new();

    for rule in rules {
        if rule_matches_element(rule, element) {
            let spec = rule.selectors.iter()
                .map(|s| {
                    let saf: u32 = s.specificity().into();
                    saf
                })
                .max()
                .unwrap_or(0);
            matched.push((rule, spec));
        }
    }

    matched.sort_by_key(|(_, spec)| *spec);

    let mut cv = ComputedValues::default();
    for (rule, _) in &matched {
        for (prop, val) in &rule.declarations {
            cv.apply(prop, val);
        }
    }
    cv
}

// --- Parsers ---

fn parse_color(value: &str) -> Option<Color> {
    let s = value.trim();
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(Color(r, g, b, 255))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Color(r, g, b, 255))
            }
            _ => None,
        }
    } else if let Some(rgb) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let mut parts = rgb.split(',').map(|p| p.trim().parse::<u8>().ok());
        let r = parts.next()??;
        let g = parts.next()??;
        let b = parts.next()??;
        Some(Color(r, g, b, 255))
    } else {
        match s {
            "red" => Some(Color(255, 0, 0, 255)),
            "green" => Some(Color(0, 128, 0, 255)),
            "blue" => Some(Color(0, 0, 255, 255)),
            "black" => Some(Color(0, 0, 0, 255)),
            "white" => Some(Color(255, 255, 255, 255)),
            "gray" | "grey" => Some(Color(128, 128, 128, 255)),
            "yellow" => Some(Color(255, 255, 0, 255)),
            "transparent" => Some(Color(0, 0, 0, 0)),
            _ => None,
        }
    }
}

fn parse_length(value: &str) -> Option<Length> {
    let s = value.trim();
    if s == "auto" {
        return Some(Length::Auto);
    }
    if let Some(px) = s.strip_suffix("px") {
        let v: f32 = px.trim().parse().ok()?;
        Some(Length::Px(v))
    } else if let Some(pt) = s.strip_suffix("pt") {
        let v: f32 = pt.trim().parse().ok()?;
        Some(Length::Px(v * 1.33333))
    } else if let Some(val) = s.strip_suffix('%') {
        if let Ok(v) = val.trim().parse::<f32>() {
            if v == 0.0 { Some(Length::Px(0.0)) } else { Some(Length::Auto) }
        } else {
            None
        }
    } else if let Ok(v) = s.parse::<f32>() {
        Some(Length::Px(v))
    } else {
        None
    }
}

fn parse_font_size(value: &str) -> Option<f32> {
    let s = value.trim();
    match s {
        "medium" => Some(16.0),
        "small" => Some(13.0),
        "large" => Some(18.0),
        "x-small" => Some(10.0),
        "x-large" => Some(24.0),
        "xx-small" => Some(7.0),
        "xx-large" => Some(32.0),
        "smaller" => Some(13.0),
        "larger" => Some(20.0),
        _ => {
            if let Some(px) = s.strip_suffix("px") {
                px.trim().parse().ok()
            } else if let Some(pt) = s.strip_suffix("pt") {
                pt.trim().parse::<f32>().ok().map(|v| v * 1.33333)
            } else if let Some(em) = s.strip_suffix("em") {
                em.trim().parse::<f32>().ok().map(|v| v * 16.0)
            } else if let Some(rem) = s.strip_suffix("rem") {
                rem.trim().parse::<f32>().ok().map(|v| v * 16.0)
            } else {
                s.parse::<f32>().ok()
            }
        }
    }
}

fn parse_display(value: &str) -> Option<Display> {
    match value.trim() {
        "block" => Some(Display::Block),
        "inline" => Some(Display::Inline),
        "none" => Some(Display::None),
        _ => None,
    }
}

fn parse_position(value: &str) -> Option<Position> {
    match value.trim() {
        "static" => Some(Position::Static),
        "relative" => Some(Position::Relative),
        "absolute" => Some(Position::Absolute),
        "fixed" => Some(Position::Fixed),
        _ => None,
    }
}

fn parse_border_width(value: &str) -> Option<f32> {
    match value.trim() {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        s => s.strip_suffix("px").and_then(|v| v.trim().parse().ok()),
    }
}

fn parse_border_style(value: &str) -> Option<BorderStyle> {
    match value.trim() {
        "none" => Some(BorderStyle::None),
        "hidden" => Some(BorderStyle::Hidden),
        "solid" => Some(BorderStyle::Solid),
        "dashed" => Some(BorderStyle::Dashed),
        "dotted" => Some(BorderStyle::Dotted),
        "double" => Some(BorderStyle::Double),
        "groove" => Some(BorderStyle::Groove),
        "ridge" => Some(BorderStyle::Ridge),
        "inset" => Some(BorderStyle::Inset),
        "outset" => Some(BorderStyle::Outset),
        _ => None,
    }
}

fn parse_overflow(value: &str) -> Option<Overflow> {
    match value.trim() {
        "visible" => Some(Overflow::Visible),
        "hidden" => Some(Overflow::Hidden),
        "scroll" => Some(Overflow::Scroll),
        "auto" => Some(Overflow::Auto),
        "clip" => Some(Overflow::Clip),
        _ => None,
    }
}

fn parse_float(value: &str) -> Option<Float> {
    match value.trim() {
        "none" => Some(Float::None),
        "left" => Some(Float::Left),
        "right" => Some(Float::Right),
        _ => None,
    }
}

fn parse_clear(value: &str) -> Option<Clear> {
    match value.trim() {
        "none" => Some(Clear::None),
        "left" => Some(Clear::Left),
        "right" => Some(Clear::Right),
        "both" => Some(Clear::Both),
        _ => None,
    }
}

fn parse_text_align(value: &str) -> Option<TextAlign> {
    match value.trim() {
        "left" => Some(TextAlign::Left),
        "right" => Some(TextAlign::Right),
        "center" => Some(TextAlign::Center),
        "justify" => Some(TextAlign::Justify),
        "start" => Some(TextAlign::Start),
        "end" => Some(TextAlign::End),
        _ => None,
    }
}

fn parse_vertical_align(value: &str) -> Option<VerticalAlign> {
    match value.trim() {
        "baseline" => Some(VerticalAlign::Baseline),
        "top" => Some(VerticalAlign::Top),
        "bottom" => Some(VerticalAlign::Bottom),
        "middle" => Some(VerticalAlign::Middle),
        "text-top" => Some(VerticalAlign::TextTop),
        "text-bottom" => Some(VerticalAlign::TextBottom),
        "sub" => Some(VerticalAlign::Sub),
        "super" => Some(VerticalAlign::Super),
        s => {
            if let Some(px) = s.strip_suffix("px") {
                px.trim().parse().ok().map(VerticalAlign::Length)
            } else {
                s.parse::<f32>().ok().map(VerticalAlign::Length)
            }
        }
    }
}

fn parse_margin_shorthand(value: &str) -> Option<(Length, Length, Length, Length)> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.len() {
        1 => {
            let v = parse_length(parts[0])?;
            Some((v.clone(), v.clone(), v.clone(), v))
        }
        2 => {
            let v = parse_length(parts[0])?;
            let h = parse_length(parts[1])?;
            Some((v.clone(), h.clone(), v, h))
        }
        4 => {
            let t = parse_length(parts[0])?;
            let r = parse_length(parts[1])?;
            let b = parse_length(parts[2])?;
            let l = parse_length(parts[3])?;
            Some((t, r, b, l))
        }
        _ => None,
    }
}
