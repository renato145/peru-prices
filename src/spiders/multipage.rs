use super::{Spider, SpiderError};
use crate::{
    configuration::{MultipageSpiderSettings, Settings},
    spiders::parse_price,
};
use anyhow::Context;
use async_trait::async_trait;
use fantoccini::{Client, ClientBuilder, Locator};
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
    time::Duration,
};
use tokio::sync::Mutex;

pub struct MultipageSpider {
    name: String,
    base_url: String,
    subroutes: Vec<String>,
    css_locator: String,
    selector: Selector,
    /// Mutex is used to lock multiple access to the webdriver
    client: Mutex<Client>,
    delay: Duration,
}

impl fmt::Display for MultipageSpider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (url={}, subroutes={})",
            self.name,
            self.base_url,
            self.subroutes.len()
        )
    }
}

impl MultipageSpider {
    pub async fn new(
        name: impl ToString,
        base_url: impl ToString,
        subroutes: Vec<impl ToString>,
        css_selector: &str,
        delay_milis: u64,
        headless: bool,
    ) -> Result<Self, SpiderError> {
        let subroutes = subroutes.into_iter().map(|x| x.to_string()).collect();
        let selector = Selector::parse(css_selector)
            .map_err(|_| SpiderError::InvalidSelector(css_selector.to_string()))?;
        let mut client = ClientBuilder::rustls();
        if headless {
            let mut caps = serde_json::map::Map::new();
            let chrome_opts = serde_json::json!({ "args": ["--headless", "--disable-gpu"] });
            caps.insert("goog:chromeOptions".to_string(), chrome_opts);
            client.capabilities(caps);
        }

        let client = client
            .connect("http://localhost:4444")
            .await
            .context("Error connecting to webdriver")?;

        Ok(Self {
            name: name.to_string(),
            base_url: base_url.to_string(),
            subroutes,
            css_locator: css_selector.to_string(),
            selector,
            client: Mutex::new(client),
            delay: Duration::from_millis(delay_milis),
        })
    }

    pub async fn from_settings(
        settings: &Settings,
        spider_settings: &MultipageSpiderSettings,
    ) -> Result<Self, SpiderError> {
        Self::new(
            spider_settings.name.clone(),
            spider_settings.base_url.clone(),
            spider_settings.subroutes.clone(),
            &spider_settings.selector,
            spider_settings.delay_milis,
            settings.headless,
        )
        .await
    }
}

#[derive(Debug, Serialize)]
pub struct MultipageItem {
    pub sku: String,
    pub name: Option<String>,
    pub brand: Option<String>,
    pub category: Option<String>,
    pub uri: Option<String>,
    pub price: Option<f64>,
}

impl PartialEq for MultipageItem {
    fn eq(&self, other: &Self) -> bool {
        self.sku == other.sku
    }
}

impl Eq for MultipageItem {
    fn assert_receiver_is_total_eq(&self) {}
}

impl Hash for MultipageItem {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.sku.hash(state);
    }
}

impl TryFrom<HashMap<String, String>> for MultipageItem {
    type Error = SpiderError;

    #[tracing::instrument(err(Debug))]
    fn try_from(mut map: HashMap<String, String>) -> Result<Self, Self::Error> {
        let sku = map.remove("data-sku").context("Failed to obtain item id")?;
        let name = map.remove("title");
        let brand = map.remove(".Showcase__brand a");
        let category = map.remove("category");
        let uri = map.remove("href");
        let price = map
            .get("data-price")
            .or_else(|| map.get(".Showcase__salePrice"))
            .map(|x| parse_price(x.as_str()))
            .transpose()?;
        if name.is_none()
            && brand.is_none()
            && category.is_none()
            && uri.is_none()
            && price.is_none()
        {
            Err(SpiderError::NoDataExtracted(format!("{:?}", map)))
        } else {
            Ok(Self {
                sku,
                name,
                brand,
                category,
                uri,
                price,
            })
        }
    }
}

#[async_trait]
impl Spider for MultipageSpider {
    type Item = MultipageItem;

    fn name(&self) -> &str {
        &self.name
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn subroutes(&self) -> &[String] {
        self.subroutes.as_slice()
    }

    fn delay(&self) -> std::time::Duration {
        self.delay
    }

    #[tracing::instrument(skip(self))]
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError> {
        let document = {
            let client = self.client.lock().await;
            client.goto(url).await.context("Failed to go to url")?;
            client
                .wait()
                .at_most(Duration::from_secs(5))
                .for_element(Locator::Css(&self.css_locator))
                .await
                .context("Failed to wait for element")?;
            client
                .source()
                .await
                .context("Failed to obtain html content")?
        };
        let html = Html::parse_document(&document);
        let elements = html
            .select(&self.selector)
            .filter_map(|element| {
                let mut map = element
                    .value()
                    .attrs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>();
                map.insert("category".to_string(), url.to_string());
                add_to_map(
                    &mut map,
                    element,
                    &[
                        (".Showcase__content", false, &["title"]),
                        (".Showcase__brand a", false, &[]),
                        (".Showcase__priceBox__title", true, &[]),
                        (".Showcase__link", false, &["href"]),
                        (".Showcase__salePrice", false, &["data-price"]),
                    ],
                );
                MultipageItem::try_from(map).ok()
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        tracing::info!("Found {} elements", elements.len());
        Ok(elements)
    }
}

/// extractros are pairs of: (class, extract_all_text, [values_to_extract])
/// If `extract_all_text` is false, only the first text inside the element will be extracted.
fn add_to_map(
    map: &mut HashMap<String, String>,
    element: ElementRef,
    extractors: &[(&str, bool, &[&str])],
) {
    extractors
        .iter()
        .for_each(|&(class, extract_all_text, values_to_extract)| {
            let selector = Selector::parse(class).unwrap();
            if let Some(child) = element.select(&selector).next() {
                let child_map = child.value().attrs().collect::<HashMap<_, _>>();
                let text = if extract_all_text {
                    child.text().collect::<String>().trim().to_string()
                } else {
                    child.text().next().unwrap_or("").trim().to_string()
                };
                map.insert(class.to_string(), text);
                values_to_extract.iter().for_each(|k| {
                    if let Some(v) = child_map.get(k) {
                        map.insert(k.to_string(), v.to_string());
                    }
                });
            }
        });
}
