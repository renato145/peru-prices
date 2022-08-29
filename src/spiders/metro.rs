use super::{Spider, SpiderError};
use anyhow::Context;
use async_trait::async_trait;
use fantoccini::{Client, ClientBuilder};
use scraper::{Html, Selector};
use std::collections::HashMap;

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

#[derive(Debug)]
pub struct MetroItem {
    pub brand: Option<String>,
    pub uri: Option<String>,
    pub name: Option<String>,
    pub price: Option<String>,
    pub category: Option<String>,
}

impl TryFrom<HashMap<&str, &str>> for MetroItem {
    type Error = SpiderError;

    fn try_from(mut map: HashMap<&str, &str>) -> Result<Self, Self::Error> {
        tracing::debug!("Received data: {:?}", map);
        let brand = map.remove("data-brand").map(String::from);
        let uri = map.remove("data-uri").map(String::from);
        let name = map.remove("data-name").map(String::from);
        let price = map.remove("data-price").map(String::from);
        let category = map.remove("data-category").map(String::from);
        if brand.is_none() && uri.is_none() {
            Err(SpiderError::NoDataExtracted(format!("{:?}", map)))
        } else {
            Ok(Self {
                brand,
                uri,
                name,
                price,
                category,
            })
        }
    }
}

#[async_trait]
impl Spider for MetroSpider {
    type Item = MetroItem;

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn subroutes(&self) -> &[String] {
        self.subroutes.as_slice()
    }

    #[tracing::instrument(skip(self))]
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError> {
        self.client.goto(url).await.context("Failed to go to url")?;
        let document = self
            .client
            .source()
            .await
            .context("Failed to obtain html content")?;
        let html = Html::parse_document(&document);
        let selector = Selector::parse(".product-item")
            .map_err(|_| SpiderError::InvalidSelector("TODO".to_string()))?;
        let elements = html
            .select(&selector)
            .filter_map(|element| {
                let map = element.value().attrs().collect::<HashMap<_, _>>();
                match MetroItem::try_from(map) {
                    Ok(item) => Some(item),
                    Err(e) => {
                        tracing::error!(error.cause_chain = ?e, error.message = %e, "Error reading item");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        tracing::info!("Found {} elements", elements.len());
        Ok(elements)
    }
}
