use markup5ever_rcdom::{Handle, NodeData};
use super::parse::{CssRule, DomElement};
use super::style::ComputedValues;
use super::stylo_integration;

#[derive(Debug)]
pub struct RenderNode {
    pub tag: String,
    pub text: String,
    pub style: ComputedValues,
    pub children: Vec<RenderNode>,
}

pub fn build(node: &Handle, rules: &[CssRule]) -> Option<RenderNode> {
    let stylist = stylo_integration::create_global_stylist();
    build_inner(node, rules, &stylist, None)
}

fn build_inner(node: &Handle, rules: &[CssRule], stylist: &style::stylist::Stylist, parent_style: Option<&ComputedValues>) -> Option<RenderNode> {
    match &node.data {
        NodeData::Document => {
            for child in node.children.borrow().iter() {
                if let Some(rc) = build_inner(child, rules, stylist, parent_style) {
                    return Some(rc);
                }
            }
            None
        }

        NodeData::Element { name, .. } => {
            let tag = name.local.to_string();

            if tag == "script" || tag == "style" {
                return None;
            }

            let element = DomElement { node: node.clone() };
            let mut cv = stylo_integration::compute_style_bridge(stylist, rules, &element);
            if let Some(parent) = parent_style {
                if cv.color == super::style::Color(0, 0, 0, 255) {
                    cv.color = parent.color.clone();
                }
                if (cv.font_size - 16.0).abs() < f32::EPSILON {
                    cv.font_size = parent.font_size;
                }
            }

            let mut children = Vec::new();
            for child in node.children.borrow().iter() {
                if let Some(rc) = build_inner(child, rules, stylist, Some(&cv)) {
                    children.push(rc);
                }
            }

            Some(RenderNode {
                tag,
                text: String::new(),
                style: cv,
                children,
            })
        }

        NodeData::Text { contents } => {
            let text = contents.borrow().trim().to_string();
            if text.is_empty() {
                return None;
            }

            let cv = parent_style.map(ComputedValues::inherit).unwrap_or_default();

            Some(RenderNode {
                tag: "#text".into(),
                text,
                style: cv,
                children: vec![],
            })
        }

        _ => None,
    }
}



pub fn dump(node: &RenderNode) {
    dump_label(node);
    for (i, child) in node.children.iter().enumerate() {
        dump_node(child, "", i == node.children.len() - 1);
    }
}

fn dump_label(node: &RenderNode) {
    if node.tag == "#text" {
        print!("{}", node.text);
    } else {
        print!("<{}", node.tag);
        print_style_attrs(&node.style);
        print!(">");
    }
}

fn print_style_attrs(style: &ComputedValues) {
    use super::style::{Display, Length, Color};

    let mut attrs: Vec<String> = Vec::new();

    if matches!(style.display, Display::Inline) {
        attrs.push("display:inline".into());
    } else if matches!(style.display, Display::None) {
        attrs.push("display:none".into());
    }

    if let Length::Px(v) = style.width {
        if v > 0.0 { attrs.push(format!("width:{}px", v)); }
    }

    if let Length::Px(v) = style.height {
        if v > 0.0 { attrs.push(format!("height:{}px", v)); }
    }

    let Color(r, g, b, _) = style.color;
    if r != 0 || g != 0 || b != 0 {
        attrs.push(format!("color:#{:02x}{:02x}{:02x}", r, g, b));
    }

    let Color(r, g, b, a) = style.background_color;
    if a > 0 && (r != 255 || g != 255 || b != 255) {
        if a == 255 {
            attrs.push(format!("bg:#{:02x}{:02x}{:02x}", r, g, b));
        } else {
            attrs.push(format!("bg:rgba({},{},{},{})", r, g, b, a));
        }
    }

    if (style.font_size - 16.0).abs() > f32::EPSILON {
        attrs.push(format!("font-size:{}px", style.font_size));
    }

    for attr in &attrs {
        print!(" {}", attr);
    }
}

fn dump_node(node: &RenderNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };

    print!("{}{}", prefix, connector);
    dump_label(node);
    println!();

    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

    for (i, child) in node.children.iter().enumerate() {
        dump_node(child, &child_prefix, i == node.children.len() - 1);
    }
}
