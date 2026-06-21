#![allow(dead_code)]
mod cache;
mod network;
mod parsing;
mod ui_shit;
mod image_handler;
mod font;

use reqwest::Client;

const BROWSER: &str = "tsfire";

fn resolve_url(base: &str, rel: &str) -> String {
    use url::Url;
    if let Ok(parsed) = Url::parse(rel) {
        return parsed.to_string();
    }
    if let Ok(base_url) = Url::parse(base) {
        if let Ok(resolved) = base_url.join(rel) {
            return resolved.to_string();
        }
    }
    rel.to_string()
}

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
    let url = std::env::args().nth(1).unwrap_or_else(|| String::from("https://example.com/"));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

        let (render_tree, decoded_images, image_map) = rt.block_on(async {
        let uabilder = network::UaBuild::new(BROWSER);
        let ua = uabilder.build();
        let client = Client::builder()
            .user_agent(ua)
            .build()?;

        let html = gget(&client, &url).await?;
        let dom = parsing::parse::phtml(&html);
        let mut css_rules = parsing::parse::collect_css(&dom);

        // fetch external stylesheets
        let external_urls = parsing::parse::collect_external_stylesheet_urls(&dom);
        for href in &external_urls {
            let abs_url = resolve_url(&url, href);
            if let Ok(css_text) = gget(&client, &abs_url).await {
                let rules = parsing::parse::parse_css(&css_text);
                css_rules.extend(rules);
            }
        }
        let render_tree = parsing::tree::build(&dom.document, &css_rules);

        // Collect <img src> URLs, fetch + decode
        let mut image_map: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
        let mut decoded_images: Vec<ui_shit::paint::DecodedImage> = Vec::new();

        fn collect_img_srcs(node: &parsing::RenderNode, urls: &mut Vec<String>) {
            if node.tag == "img" {
                if let Some(ref src) = node.src {
                    if !src.is_empty() {
                        urls.push(src.clone());
                    }
                }
            }
            for child in &node.children {
                collect_img_srcs(child, urls);
            }
        }

        if let Some(ref tree) = render_tree {
            let mut img_urls = Vec::new();
            collect_img_srcs(tree, &mut img_urls);
            img_urls.sort();
            img_urls.dedup();

            for img_url in &img_urls {
                let abs_url = resolve_url(&url, img_url);
                if let Ok(img_bytes) = gget(&client, &abs_url).await {
                    if let Ok(img) = image_handler::ImageData::decode(img_bytes.as_bytes()) {
                        let rgba = img.pixels().to_vec();
                        let idx = decoded_images.len() as u32;
                        image_map.insert(img_url.clone(), idx);
                        decoded_images.push(ui_shit::paint::DecodedImage { width: img.width(), height: img.height(), rgba });
                    }
                }
            }
        }

        use ui_shit::layout::LayoutEngine;

        if let Some(ref tree) = render_tree {
            parsing::tree::dump(tree);

            println!("\n--- layout tree ---");
            let layout_engine = ui_shit::layout::BlockLayout;
            let layout_boxes = layout_engine.layout(tree, ui_shit::layout::Size { width: 1024.0, height: 768.0 });
            ui_shit::layout::dump_boxes(&layout_boxes);

            println!();
            let dl = ui_shit::paint::build_display_list(&layout_boxes, vec![], &image_map);
            ui_shit::paint::dump_display_list(&dl);
        }

        Ok::<_, Box<dyn std::error::Error>>((render_tree, decoded_images, image_map))
    })?;

    drop(rt);
    ui_shit::window::run(render_tree, decoded_images, image_map)?;
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
