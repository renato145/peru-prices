mod metro;

pub use metro::*;

use crate::error_chain_fmt;
use async_trait::async_trait;
use futures::{stream, StreamExt};

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
    type Item: Send;

    fn base_url(&self) -> &str;
    fn subroutes(&self) -> &[String];
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError>;

    #[tracing::instrument(skip_all)]
    async fn scrape_all(&self) -> Vec<Self::Item> {
        tracing::info!("Starting scrapping on {:?}", self.base_url());
        stream::iter(self.subroutes())
            .filter_map(|subroute| async move {
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
