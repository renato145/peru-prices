use crate::{error_chain_fmt, get_peru_date, spawn_blocking_with_tracing, spiders::Spider};
use anyhow::Context;
use futures::{stream, StreamExt};
use std::{
    fmt::{Debug, Display},
    io::BufWriter,
    path::{Path, PathBuf},
};
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
    spiders: Vec<T>,
}

impl<T> Crawler<T>
where
    T: Spider + Sync + Display,
{
    pub fn new(spiders: Vec<T>) -> Self {
        Self { spiders }
    }

    /// Process all spiders and save results on `out_path`
    #[tracing::instrument(skip(self))]
    pub async fn process_all(
        self,
        out_path: impl AsRef<Path> + Debug,
        crawlers_buffer_size: usize,
        spiders_buffer_size: usize,
    ) -> Result<usize, CrawlerError> {
        let out_path = out_path.as_ref().to_path_buf();
        if !out_path.exists() {
            create_dir(&out_path)
                .await
                .context("Failed to create dir for `out_path`")?;
        } else if !out_path.is_dir() {
            return Err(CrawlerError::OutPathNoDir(out_path));
        }
        let date = get_peru_date();

        let n = stream::iter(self.spiders.into_iter())
            .map(|spider| process_one(out_path.clone(), spider, date.clone(), spiders_buffer_size))
            .buffer_unordered(crawlers_buffer_size)
            .filter_map(|res| async {
                match res {
                    Err(e) => {
                        tracing::error!(error.cause_chain = ?e, error.message = %e, "Failed to process spider");
                        None
                    }
                    Ok(n) => Some(n),
                }
            })
            .collect::<Vec<_>>()
            .await.iter().sum();

        Ok(n)
    }
}

/// Process and save results on of a spider
/// Returns the number of elements processed
#[tracing::instrument(fields(spider=%spider))]
async fn process_one<T>(
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
