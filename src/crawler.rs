use crate::{
    configuration::Settings, error_chain_fmt, get_peru_date, spawn_blocking_with_tracing,
    spiders::Spider,
};
use anyhow::Context;
use std::{fmt::Display, io::BufWriter, path::PathBuf};
use tokio::{
    fs::{create_dir, File},
    time::Instant,
};

#[derive(thiserror::Error)]
pub enum CrawlerError {
    #[error("Provided out_path is not a directory: {0}")]
    OutPathNoDir(PathBuf),
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
    T: Spider + Sync + Display,
{
    spider: T,
    path: PathBuf,
    buffer_size: usize,
}

impl<T> Crawler<T>
where
    T: Spider + Sync + Display,
{
    pub fn new(spider: T, configuration: &Settings) -> Self {
        Self {
            spider,
            path: configuration.out_path.clone(),
            buffer_size: configuration.crawlers_buffer_size,
        }
    }

    /// Process spider and save results on `out_path`
    #[tracing::instrument(skip(self), fields(path=?self.path, buffer_size=self.buffer_size))]
    pub async fn process(self) -> Result<usize, CrawlerError> {
        if !self.path.exists() {
            create_dir(&self.path)
                .await
                .context("Failed to create dir for `out_path`")?;
        } else if !self.path.is_dir() {
            return Err(CrawlerError::OutPathNoDir(self.path));
        }
        let date = get_peru_date();
        let n = match process_spider(self.path, self.spider, date, self.buffer_size).await {
            Err(e) => {
                tracing::error!(error.cause_chain = ?e, error.message = %e, "Failed to process spider");
                0
            }
            Ok(n) => n,
        };

        Ok(n)
    }
}

/// Process and save results on of a spider
/// Returns the number of elements processed
#[tracing::instrument(fields(spider=%spider))]
async fn process_spider<T>(
    out_path: PathBuf,
    spider: T,
    date: String,
    spiders_buffer_size: usize,
) -> Result<usize, CrawlerError>
where
    T: Spider + Sync + Display,
{
    tracing::info!("Start scrapping");
    let now = Instant::now();
    let mut path = out_path.clone();
    path.push(format!("{}_{}.csv", spider.name(), date));
    let file = File::create(path)
        .await
        .context("Failed to create file")?
        .into_std()
        .await;
    let items = spider.scrape_all(spiders_buffer_size).await;
    let n = items.len();
    spawn_blocking_with_tracing(move || {
        let mut wtr = csv::Writer::from_writer(BufWriter::new(file));
        items.into_iter().for_each(|item| {
            wtr.serialize(item).unwrap();
        });
    })
    .await
    .context("Failed to join task")?;
    tracing::info!("Scraped {} elements in {:?}", n, now.elapsed());
    Ok(n)
}
