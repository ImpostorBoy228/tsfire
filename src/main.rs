mod network;
mod parse;
mod render;
mod style;
mod layout;
mod paint;

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
