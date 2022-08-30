use super::{Spider, SpiderError};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use fantoccini::{Client, ClientBuilder, Locator};
use scraper::{Html, Selector};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
    time::Duration,
};
use tokio::time::sleep;

pub struct MetroSpider {
    name: String,
    base_url: String,
    subroutes: Vec<String>,
    selector: Selector,
    client: Client,
    delay: Duration,
    /// Delay after scroll down
    scroll_delay: Duration,
    /// Number of checks before finishing to scroll down
    scroll_checks: usize,
}

impl fmt::Display for MetroSpider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MetroSpider ({}, subroutes={})",
            self.name,
            self.subroutes.len()
        )
    }
}

impl MetroSpider {
    pub async fn new(
        name: impl ToString,
        base_url: impl ToString,
        subroutes: Vec<impl ToString>,
        selector: &str,
        delay_milis: u64,
        scroll_delay_milis: u64,
        scroll_checks: usize,
    ) -> Result<Self, SpiderError> {
        let subroutes = subroutes.into_iter().map(|x| x.to_string()).collect();
        let selector = Selector::parse(selector)
            .map_err(|_| SpiderError::InvalidSelector(selector.to_string()))?;
        let mut caps = serde_json::map::Map::new();
        let chrome_opts = serde_json::json!({ "args": ["--headless", "--disable-gpu"] });
        caps.insert("goog:chromeOptions".to_string(), chrome_opts);
        let client = ClientBuilder::rustls()
            .capabilities(caps)
            .connect("http://localhost:4444")
            .await
            .context("Error connecting to webdriver")?;
        Ok(Self {
            name: name.to_string(),
            base_url: base_url.to_string(),
            subroutes,
            selector,
            client,
            delay: Duration::from_millis(delay_milis),
            scroll_delay: Duration::from_millis(scroll_delay_milis),
            scroll_checks,
        })
    }

    async fn get_height(&self) -> Result<i64, SpiderError> {
        let value = self
            .client
            .execute("return document.body.scrollHeight", vec![])
            .await
            .context("Failed to get height")?;
        let current_height = value
            .as_i64()
            .ok_or_else(|| anyhow!("No number found: {}", value))?;
        Ok(current_height)
    }

    #[tracing::instrument(skip_all)]
    async fn scroll_down(&self) -> Result<(), SpiderError> {
        tracing::debug!("Scrolling down");
        self.client
            .execute("window.scrollTo(0, document.body.scrollHeight);", vec![])
            .await
            .context("Failed to scroll down")?;
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn scroll_to_end(&self) -> Result<(), SpiderError> {
        let mut height = self.get_height().await?;
        tracing::debug!("height={}", height);
        let mut i = 0;
        loop {
            self.scroll_down().await?;
            sleep(self.scroll_delay).await;
            let new_height = self.get_height().await?;
            tracing::debug!("new_height={}", new_height);
            if new_height == height {
                i += 1;
            }
            if i >= self.scroll_checks {
                tracing::debug!("scroll_checks={}", i);
                break;
            }
            height = new_height;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct MetroItem {
    pub id: String,
    pub brand: Option<String>,
    pub uri: Option<String>,
    pub name: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
}

impl PartialEq for MetroItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MetroItem {
    fn assert_receiver_is_total_eq(&self) {}
}

impl Hash for MetroItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl TryFrom<HashMap<&str, &str>> for MetroItem {
    type Error = SpiderError;

    fn try_from(mut map: HashMap<&str, &str>) -> Result<Self, Self::Error> {
        tracing::debug!("Received data: {:?}", map);
        let id = map
            .remove("data-id")
            .map(String::from)
            .context("Failed to obtain item id")?;
        let brand = map.remove("data-brand").map(String::from);
        let uri = map.remove("data-uri").map(String::from);
        let name = map.remove("data-name").map(String::from);
        let price = map
            .remove("data-price")
            .map(|x| {
                x.replace("S/.", "")
                    .replace(',', "")
                    .trim()
                    .parse()
                    .with_context(|| format!("Failed to parse price from: {:?}", x))
            })
            .transpose()?;
        let category = map.remove("data-category").map(String::from);
        if brand.is_none()
            && uri.is_none()
            && name.is_none()
            && price.is_none()
            && category.is_none()
        {
            Err(SpiderError::NoDataExtracted(format!("{:?}", map)))
        } else {
            Ok(Self {
                id,
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

    fn name(&self) -> &str {
        &self.name
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn subroutes(&self) -> &[String] {
        self.subroutes.as_slice()
    }

    fn delay(&self) -> Duration {
        self.delay
    }

    #[tracing::instrument(skip(self))]
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError> {
        self.client.goto(url).await.context("Failed to go to url")?;
        self.client
            .wait()
            .at_most(Duration::from_secs(5))
            .for_element(Locator::Css(".product-item"))
            .await
            .context("Failed to wait for element")?;
        if let Err(e) = self.scroll_to_end().await {
            tracing::error!(error.cause_chain = ?e, error.message = %e, "Failed to scroll to end");
        }
        let document = self
            .client
            .source()
            .await
            .context("Failed to obtain html content")?;
        let html = Html::parse_document(&document);
        let elements = html
            .select(&self.selector)
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
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        tracing::info!("Found {} elements", elements.len());
        Ok(elements)
    }
}
