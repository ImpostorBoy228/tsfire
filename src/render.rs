use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

#[derive(Debug)]
pub struct RenderNode {
    pub tag: String,
    pub text: String,
    pub styles: HashMap<String, String>,
    pub children: Vec<RenderNode>,
}

pub fn build(node: &Handle) -> Option<RenderNode> {
    match &node.data {
        NodeData::Document => {
            for child in node.children.borrow().iter() {
                if let Some(rc) = build(child) {
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

            let mut children = Vec::new();
            for child in node.children.borrow().iter() {
                if let Some(rc) = build(child) {
                    children.push(rc);
                }
            }

            Some(RenderNode {
                tag,
                text: String::new(),
                styles: HashMap::new(),
                children,
            })
        }

        NodeData::Text { contents } => {
            let text = contents.borrow().trim().to_string();
            if text.is_empty() {
                return None;
            }

            Some(RenderNode {
                tag: "#text".into(),
                text,
                styles: HashMap::new(),
                children: vec![],
            })
        }

        _ => None,
    }
}

pub fn dump(node: &RenderNode) {
    println!("{}", node.tag);
    for (i, child) in node.children.iter().enumerate() {
        dump_node(child, "", i == node.children.len() - 1);
    }
}

fn dump_node(node: &RenderNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };

    let label = if node.tag == "#text" {
        &node.text
    } else {
        &node.tag
    };

    println!("{}{}{}", prefix, connector, label);

    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

    for (i, child) in node.children.iter().enumerate() {
        dump_node(child, &child_prefix, i == node.children.len() - 1);
    }
}
