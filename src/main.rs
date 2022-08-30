use peru_prices::{configuration::get_configuration, crawler::Crawler, spiders::MetroSpider};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let configuration = get_configuration().expect("Failed to get configuration");
    tracing::info!("Initializing scrappers...");
    tracing::debug!("{:#?}", configuration);

    let spider = MetroSpider::new(
        configuration.metro.name,
        configuration.metro.base_url,
        configuration.metro.subroutes,
        &configuration.metro.selector,
        configuration.metro.delay_milis,
        configuration.metro.scroll_delay_milis,
        configuration.metro.scroll_checks,
        configuration.metro.headless,
    )
    .await?;
    let crawler = Crawler::new(vec![spider]);
    crawler.process_all(configuration.out_path, configuration.crawlers_buffer_size).await?;
    Ok(())
}
