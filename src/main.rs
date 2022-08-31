use peru_prices::{
    configuration::get_configuration, crawler::Crawler, spiders::InfiniteScrollingSpider,
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

    let spiders = vec![
        InfiniteScrollingSpider::from_settings(
            configuration.metro,
            &configuration.infinite_scrolling,
        )
        .await?,
        InfiniteScrollingSpider::from_settings(
            configuration.wong,
            &configuration.infinite_scrolling,
        )
        .await?,
        // InfiniteScrollingSpider::from_settings(
        //     configuration.plaza_vea,
        //     &configuration.infinite_scrolling,
        // )
        // .await?,
    ];
    let crawler = Crawler::new(spiders);
    let n = crawler
        .process_all(
            configuration.out_path,
            configuration.crawlers_buffer_size,
            configuration.spiders_buffer_size,
        )
        .await?;
    tracing::info!("Finished in {:?} ({} items)", now.elapsed(), n);
    Ok(())
}
