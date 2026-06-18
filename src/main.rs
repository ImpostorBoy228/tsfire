mod network;
mod parse;
mod render;
mod style;
mod layout;
mod paint;
mod stylo_integration;

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
    println!("ua: {}\n\n", ua);
    let client = Client::builder()
        .user_agent(ua)
        .build()?;
    // fuck ua tech

    let response = gget(&client, "https://wikipedia.org").await?;

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
    use ::style::properties::PropertyDeclarationBlock;
    use ::style::shared_lock::{SharedRwLock, StylesheetGuards};
    use ::style::servo_arc::Arc;
    
    #[test]
    fn test_stylo_integration() {
        // Create a stylist
        let stylist = stylo_integration::create_global_stylist();
        
        // Create a lock and use it for both the PDB and in compute_style
        let lock = SharedRwLock::new();
        let pdb = Arc::new(lock.wrap(PropertyDeclarationBlock::new()));
        
        // Compute style with no parent
        let guard = lock.read();
        let guards = StylesheetGuards::same(&guard);
        let computed = stylist.compute_for_declarations::<stylo_integration::PhantomElement>(
            &guards, 
            stylist.device().default_computed_values(), 
            pdb
        );
        
        // Verify we got a computed values object
        // Just check that we can access some field - display is a good one
        assert!(computed.get_box().display != ::style::values::computed::Display::None);
    }
}
