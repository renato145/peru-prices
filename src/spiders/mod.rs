mod metro;
pub use metro::*;
use serde::Serialize;
use tokio::time::sleep;

use crate::error_chain_fmt;
use async_trait::async_trait;
use futures::{stream, StreamExt};
use std::time::Duration;

#[derive(thiserror::Error)]
pub enum SpiderError {
    #[error("Invalid selector: {0}")]
    InvalidSelector(String),
    #[error("No data found to be extracted: {0}")]
    NoDataExtracted(String),
    #[error("Something went wrong.")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SpiderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[async_trait]
pub trait Spider {
    type Item: Send + Sync + std::fmt::Debug + Serialize + 'static;

    fn name(&self) -> &str;
    fn base_url(&self) -> &str;
    fn subroutes(&self) -> &[String];
    /// Delay to scrap between subroutes
    fn delay(&self) -> Duration;
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError>;

    #[tracing::instrument(skip_all)]
    async fn scrape_all(&self) -> Vec<Self::Item> {
        tracing::info!("Start scrapping on {:?}", self.base_url());
        stream::iter(self.subroutes())
            .enumerate()
            .filter_map(|(i, subroute)| async move {
                if i > 0 {
                    sleep(self.delay()).await;
                }
                let subroute = format!("{}/{}", self.base_url(), subroute);
                match self.scrape(&subroute).await {
                    Ok(item) => Some(item),
                    Err(e) => {
                        tracing::error!(error.cause_chain = ?e,
                                        error.message = %e,
                                        "Failed to scrape {:?}", subroute);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect()
    }
}
