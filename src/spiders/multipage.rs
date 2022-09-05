use super::{Spider, SpiderError};
use crate::{configuration::MultipageSpiderSettings, spiders::parse_price};
use anyhow::Context;
use async_trait::async_trait;
use reqwest::{header::USER_AGENT, Client};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    hash::Hash,
    time::Duration,
};

pub struct MultipageSpider {
    name: String,
    base_url: String,
    subroutes: Vec<String>,
    selector: Selector,
    client: ClientWithMiddleware,
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
    pub fn new(
        name: impl ToString,
        base_url: impl ToString,
        subroutes: Vec<impl ToString>,
        css_selector: &str,
        delay_milis: u64,
    ) -> Result<Self, SpiderError> {
        let subroutes = subroutes.into_iter().map(|x| x.to_string()).collect();
        let selector = Selector::parse(css_selector)
            .map_err(|_| SpiderError::InvalidSelector(css_selector.to_string()))?;
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(Client::new())
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            name: name.to_string(),
            base_url: base_url.to_string(),
            subroutes,
            selector,
            client,
            delay: Duration::from_millis(delay_milis),
        })
    }

    pub fn from_settings(settings: &MultipageSpiderSettings) -> Result<Self, SpiderError> {
        Self::new(
            settings.name.clone(),
            settings.base_url.clone(),
            settings.subroutes.clone(),
            &settings.selector,
            settings.delay_milis,
        )
    }
}

#[derive(Debug, Serialize)]
pub struct MultipageItem {
    pub sku: String,
    pub name: Option<String>,
    pub brand: Option<String>,
    pub department: Option<String>,
    pub category: Option<String>,
    pub price_box: Option<String>,
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

    fn try_from(mut map: HashMap<String, String>) -> Result<Self, Self::Error> {
        tracing::debug!("Received data: {:#?}", map);
        let sku = map.remove("data-sku").context("Failed to obtain item id")?;
        let name = map.remove("title");
        let brand = map.remove(".Showcase__brand a");
        let department = map.remove("data-dep");
        let category = map.remove("data-cat");
        let price_box = map.remove(".Showcase__priceBox__title");
        let uri = map.remove("href");
        let price = map
            .get(".Showcase__salePrice")
            .map(|x| parse_price(x.as_str()))
            .transpose()?;
        if name.is_none()
            && brand.is_none()
            && department.is_none()
            && category.is_none()
            && price_box.is_none()
            && uri.is_none()
            && price.is_none()
        {
            Err(SpiderError::NoDataExtracted(format!("{:?}", map)))
        } else {
            Ok(Self {
                sku,
                name,
                brand,
                department,
                category,
                price_box,
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
        let document = self
            .client
            .get(url)
            .header(
                USER_AGENT,
                "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:47.0) Gecko/20100101 Firefox/47.0",
            )
            .send()
            .await
            .context("Failed to send request")?
            .text()
            .await
            .context("Failed to read document")?;
        let html = Html::parse_document(&document);
        let elements = html
            .select(&self.selector)
            .filter_map(|element| {
                let mut map = element
                    .value()
                    .attrs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<HashMap<_, _>>();
                add_to_map(
                    &mut map,
                    element,
                    &[
                        (".Showcase__content", false, &["title"]),
                        (".Showcase__brand a", false, &[]),
                        (".Showcase__priceBox__title", true, &[]),
                        (".Showcase__link", false, &["href"]),
                        (".Showcase__salePrice", false, &[]),
                    ],
                );
                match MultipageItem::try_from(map) {
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
            let child = element.select(&selector).next().unwrap();
            let child_map = child.value().attrs().collect::<HashMap<_, _>>();
            let text = if extract_all_text {
                child.text().collect::<String>().trim().to_string()
            } else {
                child.text().next().unwrap_or("").trim().to_string()
            };
            map.insert(class.to_string(), text);
            values_to_extract.iter().for_each(|k| {
                let v = child_map.get(k).unwrap_or(&"").to_string();
                map.insert(k.to_string(), v);
            });
        });
}
