use super::{Spider, SpiderError};
use crate::configuration::MultipageSpiderSettings;
use anyhow::Context;
use async_trait::async_trait;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use scraper::{Html, Selector};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fmt,
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

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
pub struct MultipageItem {}

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
                let map = element.value().attrs().collect::<HashMap<_, _>>();
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
