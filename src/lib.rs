pub mod configuration;
pub mod crawler;
pub mod spiders;

use chrono::{FixedOffset, Utc};
use tokio::task::JoinHandle;

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

/// Gets current date on "%Y%m%d" format for Peru timezone
pub fn get_peru_date() -> String {
    Utc::now()
        .with_timezone(&FixedOffset::west(5 * 3600))
        .format("%Y%m%d")
        .to_string()
}

pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || current_span.in_scope(f))
}
