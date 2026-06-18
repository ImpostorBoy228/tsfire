use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, RcDom};

pub fn phtml(html: &str) -> RcDom {
    parse_document(RcDom::default(), Default::default())
        .one(html)
}

pub fn walk<F>(node: &Handle, visitor: &mut F)
where
    F: FnMut(&Handle),
{
    visitor(node);

    for child in node.children.borrow().iter() {
        walk(child, visitor);
    }
}
