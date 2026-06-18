#![allow(dead_code)]
mod network;
mod parse;
mod render;
mod style;
mod layout;
mod paint;
mod stylo_integration;
mod image_handler;
mod font;

use reqwest::Client;

const BROWSER: &str = "tsfire";

async fn gget(client: &Client, url: &str) ->
    Result<String, reqwest::Error> {
    let body = client.get(url)
        .send()
        .await?
        .text()
        .await?;
    Ok(body)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let uabilder = network::UaBuild::new(BROWSER);
    let ua = uabilder.build();
    let client = Client::builder()
        .user_agent(ua)
        .build()?;

    let response = gget(&client, "https://example.com/").await?;

    let dom = parse::phtml(&response);

    let css_rules = parse::collect_css(&dom);

    let render_tree = render::build(&dom.document, &css_rules);

    use layout::LayoutEngine;

    if let Some(tree) = render_tree {
        render::dump(&tree);

        println!("\n--- layout tree ---");
        let layout_engine = layout::BlockLayout;
        let layout_boxes = layout_engine.layout(&tree, layout::Size { width: 1024.0, height: 768.0 });
        layout::dump_boxes(&layout_boxes);

        println!();
        let dl = paint::build_display_list(&layout_boxes);
        paint::dump_display_list(&dl);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stylo_integration() {
        let stylist = stylo_integration::create_global_stylist();
        let lock = ::style::shared_lock::SharedRwLock::new();
        let pdb = ::style::servo_arc::Arc::new(
            lock.wrap(::style::properties::PropertyDeclarationBlock::new())
        );
        let guard = lock.read();
        let guards = ::style::shared_lock::StylesheetGuards::same(&guard);
        let computed = stylist.compute_for_declarations::<stylo_integration::PhantomElement>(
            &guards,
            stylist.device().default_computed_values(),
            pdb
        );
        assert!(computed.get_box().display != ::style::values::computed::Display::None);
    }
}