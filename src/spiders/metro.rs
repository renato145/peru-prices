use super::{Spider, SpiderError};
use anyhow::Context;
use async_trait::async_trait;
use fantoccini::{Client, ClientBuilder};

pub struct MetroSpider {
    base_url: String,
    subroutes: Vec<String>,
    client: Client,
}

impl MetroSpider {
    pub async fn new(base_url: String, subroutes: Vec<String>) -> Result<Self, SpiderError> {
        let mut caps = serde_json::map::Map::new();
        let chrome_opts = serde_json::json!({ "args": ["--headless", "--disable-gpu"] });
        caps.insert("goog:chromeOptions".to_string(), chrome_opts);
        let client = ClientBuilder::rustls()
            .capabilities(caps)
            .connect("http://localhost:4444")
            .await
            .context("Error connecting to webdriver")?;
        Ok(Self {
            base_url,
            subroutes,
            client,
        })
    }
}

pub struct MetroItem;

#[async_trait]
impl Spider for MetroSpider {
    type Item = MetroItem;

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn subroutes(&self) -> &[String] {
        self.subroutes.as_slice()
    }

    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError> {
        self.client.goto(url).await.context("Failed to go to url")?;
        let html = self
            .client
            .source()
            .await
            .context("Failed to obtain html content")?;
        println!("{:?}", html);
        Ok(vec![MetroItem])
    }
}
