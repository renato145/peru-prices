use futures::future::join_all;
use peru_prices::{
    configuration::get_configuration,
    crawler::Crawler,
    spiders::{InfiniteScrollingSpider, MultipageSpider},
};
use tokio::time::Instant;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_file(true)
                .with_line_number(true),
        )
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let configuration = get_configuration().expect("Failed to get configuration");
    tracing::info!("Initializing scrappers...");
    tracing::debug!("{:#?}", configuration);
    let now = Instant::now();

    let metro_spider = InfiniteScrollingSpider::from_settings(
        &configuration.metro,
        &configuration.infinite_scrolling,
    )
    .await?;
    let wong_spider = InfiniteScrollingSpider::from_settings(
        &configuration.wong,
        &configuration.infinite_scrolling,
    )
    .await?;
    let plaza_vea_spider = MultipageSpider::from_settings(&configuration.plaza_vea)?;

    let tasks = vec![
        // tokio::spawn(Crawler::new(metro_spider, &configuration).process()),
        // tokio::spawn(Crawler::new(wong_spider, &configuration).process()),
        tokio::spawn(Crawler::new(plaza_vea_spider, &configuration).process()),
    ];

    let n: usize = join_all(tasks).await.into_iter().map(|res| match res {
        Ok(Ok(n)) => n,
        Err(e) => {
            tracing::error!(error.cause_chain = ?e, error.message = %e, "Failed to execute task");
            0
        }
        _ => 0,
    }).sum();

    tracing::info!("Finished in {:?} ({} items)", now.elapsed(), n);
    Ok(())
}
