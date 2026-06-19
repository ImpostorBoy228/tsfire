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
        }
    }
}

impl ComputedValues {
    pub fn inherit(parent: &ComputedValues) -> Self {
        let mut cv = ComputedValues::default();
        cv.color = parent.color.clone();
        cv.font_size = parent.font_size;
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
