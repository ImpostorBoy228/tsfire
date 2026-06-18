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

    // Сбор CSS из <style>
    let mut css_buf = String::new();
    parse::walk(&dom.document, &mut |node| {
        if let NodeData::Element { name, .. } = &node.borrow().data {
            if name.local.as_ref() == "style" {
                let content = parse::element_text_contents(node);
                if !content.is_empty() {
                    css_buf.push_str(&content);
                    css_buf.push('\n');
                }
            }
        }
    });

    let rules = parse::parse_css(&css_buf);
    println!("Parsed {} CSS rules.", rules.len());


    let mut element_styles: Vec<(String, HashMap<String, String>)> = Vec::new();

    parse::walk(&dom.document, &mut |node| {
        if let NodeData::Element { name, .. } = &node.borrow().data {
            let element = parse::DomElement { node };
            let mut styles = HashMap::new();

            let mut applicable_rules: Vec<(usize, &parse::CssRule)> = Vec::new();
            for rule in &rules {
                if parse::rule_matches_element(rule, &element) {
                    let specificity = rule.selectors.iter()
                        .map(|s| s.specificity())
                        .max()
                        .unwrap_or(0);
                    applicable_rules.push((specificity, rule));
                }
            }

            applicable_rules.sort_by_key(|(spec, _)| *spec);
            for (_, rule) in applicable_rules {
                for (k, v) in &rule.declarations {
                    styles.insert(k.clone(), v.clone());
                }
            }

            // Добавляем инлайн-стили (если есть)
            if let Some(style_attr) = element.get_attr("style") {
                // Упрощённо: парсим строку вида "prop: val; prop2: val2;"
                for part in style_attr.split(';') {
                    if let Some(eq_pos) = part.find(':') {
                        let name = part[..eq_pos].trim().to_string();
                        let value = part[eq_pos+1..].trim().to_string();
                        if !name.is_empty() {
                            styles.insert(name, value);
                        }
                    }
                }
            }

            let tag = name.local.as_ref().to_string();
            element_styles.push((tag, styles));
        }
    });

    // Выводим стили для первых 10 элементов
    for (i, (tag, styles)) in element_styles.iter().enumerate().take(10) {
        println!("[{}] <{}> styles: {:?}", i, tag, styles);
    }

    Ok(())
}
