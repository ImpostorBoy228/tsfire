mod network;
mod parse;

use reqwest::{Client};//, Error};

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

    let uabilder = network::uaBuild::new(BROWSER);
    let ua = uabilder.build();
    println!("ua: {}\n\n", ua);
    let client = Client::builder()
        .user_agent(ua)
        .build()?;
// fuck ua tech

    let response = gget(&client, "https://wikipedia.org").await?;

    // println!("{}", &response);

    let dom = parse::phtml(&response);
    parse::walk(&dom.document, &mut |node|
        {
        println!("{:?}", node.data);
        }
    );

    Ok(())
}
