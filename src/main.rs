use reqwest::{Client}; // Error};

//classlike shit
pub struct uaBuild {
    app_name: String,
    app_version: String,
}

impl uaBuild{
    pub fn new(app_name: &str) -> Self {
        Self {
            app_name: app_name.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn getos() -> &'static str {
        #[cfg(target_os = "windows")]
        { "Windows NT 10.0; Win64; x64" }

        #[cfg(target_os = "macos")]
        { "Macintosh; Intel Mac OS X 10_15_7" }

        #[cfg(target_os = "linux")]
        { "X11; Linux x86_64" }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        { "Unknown OS" }
    }

    pub fn build(&self) -> String {
        let chrome_version = "131.2.2.8";
        let safari_version = "537.1488";
        let os_info = Self::getos();

        format!(
            "Mozilla/5.0 ({}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{} Safari/{} {}/{}",
            os_info, chrome_version, safari_version, self.app_name, self.app_version
        )
    }
}       //🖕🖕🖕🖕

async fn gget(client: &Client, url: &str) -> Result<String, reqwest::Error> {
    let body = client.get(url)
        .send()
        .await?
        .text()
        .await?;

    Ok(body)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let uabilder = uaBuild::new("tsfire");
    let ua = uabilder.build();
    println!("ua: {}", ua);
    let client = Client::builder()
        .user_agent(ua)
        .build()?;
// fuck ua tech

    let response = gget(&client, "https://wikipedia.org").await?;

    println!("{}", &response);
    Ok(())
}
