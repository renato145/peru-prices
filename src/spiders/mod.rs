mod infinite_scrolling;
mod multipage;
use anyhow::Context;
pub use infinite_scrolling::*;
pub use multipage::*;

use crate::error_chain_fmt;
use async_trait::async_trait;
use futures::{stream, StreamExt};
use serde::Serialize;
use std::{collections::HashSet, hash::Hash, time::Duration};
use tokio::time::sleep;

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
    type Item: std::fmt::Debug + Eq + Hash + Send + Sync + Serialize + 'static;

    fn name(&self) -> &str;
    fn base_url(&self) -> &str;
    fn subroutes(&self) -> &[String];
    /// Delay to scrap between subroutes
    fn delay(&self) -> Duration;
    async fn scrape(&self, url: &str) -> Result<Vec<Self::Item>, SpiderError>;

    #[tracing::instrument(skip(self))]
    async fn scrape_all(&self, spiders_buffer_size: usize) -> Vec<Self::Item> {
        stream::iter(self.subroutes().iter().cloned())
            .enumerate()
            .map(|(i, subroute)| async move {
                if i > 0 {
                    sleep(self.delay()).await;
                }
                let subroute = format!("{}/{}", self.base_url(), subroute);
                self.scrape(&subroute).await
            })
            .buffer_unordered(spiders_buffer_size)
            .filter_map(|res| async {
                match res {
                    Ok(items) => Some(items),
                    Err(e) => {
                        tracing::error!(error.cause_chain = ?e,
                                        error.message = %e,
                                        "Failed to scrape subroute");
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect()
    }
}

pub fn parse_price(x: &str) -> Result<f64, SpiderError> {
    let price = x
        .replace("S/.", "")
        .replace("S/", "")
        .replace(',', "")
        .trim()
        .parse::<f64>()
        .with_context(|| format!("Failed to parse price from: {:?}", x))?;
    Ok(price)
}
