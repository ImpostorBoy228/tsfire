mod network;
mod parse;

use reqwest::Client;
use markup5ever_rcdom::NodeData;

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

    let mut css_buf = String::new();

    parse::walk(&dom.document, &mut |node|
        {
        match &node.data {
            NodeData::Element { name, .. } => {
                if name.local.as_ref() == "style" {
                    for child in node.children.borrow().iter() {
                        if let NodeData::Text { contents } = &child.data {
                            let content = contents.borrow();
                            if !content.is_empty() {
                                css_buf.push_str(&content);
                                css_buf.push('\n');
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    });

    let rules = parse::parse_css(&css_buf);
    println!("Parsed {} CSS rules.", rules.len());

    for (i, rule) in rules.iter().take(5).enumerate() {
        println!("Rule {}: selectors: {:?}", i, rule.selectors);
        println!("  declarations: {:?}", rule.declarations);
    }

    Ok(())
}
