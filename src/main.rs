use peru_prices::{crawler::Crawler, spiders::MetroSpider};
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    tracing::info!("Initializing scrappers...");

    let subroutes = vec![
        "frutas-y-verduras",
        // "carnes-aves-y-pescados",
        // "desayuno",
        // "lacteos",
        // "embutidos-y-fiambres",
        // "abarrotes",
        // "panaderia-y-pasteleria",
        // "comidas-y-rostizados",
        // "congelados",
        // "aguas-y-bebidas",
        // "cervezas-vinos-y-licores",
        // "limpieza",
        // "higiene-salud-y-belleza",
    ];
    let spider = MetroSpider::new(
        "metro",
        "https://www.metro.pe",
        subroutes,
        ".product-item",
        500,
        250,
        20,
    )
    .await?;
    let crawler = Crawler::new(vec![spider]);
    crawler.process_all("output").await?;
    Ok(())
}
