use super::{Spider, SpiderError};
use async_trait::async_trait;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use reqwest_tracing::TracingMiddleware;
use std::time::Duration;

pub struct MetroSpider {
    base_url: String,
    subroutes: Vec<String>,
    client: ClientWithMiddleware,
}

impl MetroSpider {
    pub fn new(base_url: String, subroutes: Vec<String>) -> Self {
        let client = ClientBuilder::new(Client::new())
            .with(TracingMiddleware::default())
            .with(RetryTransientMiddleware::new_with_policy(
                ExponentialBackoff::builder().build_with_max_retries(3),
            ))
            .build();
        Self {
            base_url,
            subroutes,
            client,
        }
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
        let body = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(SpiderError::RequestError)?
            .text()
            .await
            .map_err(SpiderError::DecodeHtmlError)?;
        todo!()
    }
}
