use crate::{error_chain_fmt, spiders::Spider};

#[derive(thiserror::Error)]
pub enum CrawlerError {
    #[error("Something went wrong.")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for CrawlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub struct Crawler<T>
where
    T: Spider + Sync,
{
    spiders: Vec<T>,
}

impl<T> Crawler<T>
where
    T: Spider + Sync,
{
    pub fn new(spiders: Vec<T>) -> Self {
        Self { spiders }
    }

    #[tracing::instrument(skip_all)]
    pub async fn process_all(&self) {
        for spider in &self.spiders {
            let items = spider.scrape_all().await;
            items.into_iter().for_each(|item| println!("{:?}", item));
        }
    }
}
