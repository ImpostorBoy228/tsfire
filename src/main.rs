#![allow(dead_code)]
mod network;
mod parsing;
mod ui_shit;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let dl = rt.block_on(async {
        let uabilder = network::UaBuild::new(BROWSER);
        let ua = uabilder.build();
        let client = Client::builder()
            .user_agent(ua)
            .build()?;

        let response = gget(&client, "https://example.com/").await?;
        let dom = parsing::parse::phtml(&response);
        let css_rules = parsing::parse::collect_css(&dom);
        let render_tree = parsing::tree::build(&dom.document, &css_rules);

        use ui_shit::layout::LayoutEngine;

        let dl = if let Some(tree) = render_tree {
            parsing::tree::dump(&tree);

            println!("\n--- layout tree ---");
            let layout_engine = ui_shit::layout::BlockLayout;
            let layout_boxes = layout_engine.layout(&tree, ui_shit::layout::Size { width: 1024.0, height: 768.0 });
            ui_shit::layout::dump_boxes(&layout_boxes);

            println!();
            let dl = ui_shit::paint::build_display_list(&layout_boxes);
            ui_shit::paint::dump_display_list(&dl);
            dl
        } else {
            ui_shit::paint::DisplayList {
                items: vec![],
                text_arena: String::new(),
                images: vec![],
                content_size: ui_shit::layout::Size { width: 1024.0, height: 768.0 },
            }
        };

        Ok::<_, Box<dyn std::error::Error>>(dl)
    })?;

    drop(rt);
    ui_shit::window::run(dl)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stylo_integration() {
        let stylist = parsing::stylo_integration::create_global_stylist();
        let lock = ::style::shared_lock::SharedRwLock::new();
        let pdb = ::style::servo_arc::Arc::new(
            lock.wrap(::style::properties::PropertyDeclarationBlock::new())
        );
        let guard = lock.read();
        let guards = ::style::shared_lock::StylesheetGuards::same(&guard);
        let computed = stylist.compute_for_declarations::<parsing::stylo_integration::PhantomElement>(
            &guards,
            stylist.device().default_computed_values(),
            pdb
        );
        assert!(computed.get_box().display != ::style::values::computed::Display::None);
    }
}
