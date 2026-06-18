mod network;
mod parse;
mod render;

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

    let render_tree = render::build(&dom.document);

    if let Some(tree) = render_tree {
        render::dump(&tree);
    }

    Ok(())
}
