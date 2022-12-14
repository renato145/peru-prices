use config::Config;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub out_path: PathBuf,
    pub headless: bool,
    pub delay_milis: u64,
    pub spiders_buffer_size: usize,
    pub infinite_scrolling: InfiniteScrollingSettings,
    pub metro: InfiniteScrollingSpiderSettings,
    pub wong: InfiniteScrollingSpiderSettings,
    pub plaza_vea: MultipageSpiderSettings,
}

#[derive(Debug, Deserialize)]
pub struct InfiniteScrollingSettings {
    pub scroll_delay_milis: u64,
    pub scroll_checks: usize,
}

#[derive(Debug, Deserialize)]
pub struct InfiniteScrollingSpiderSettings {
    pub name: String,
    pub base_url: String,
    pub subroutes: Vec<String>,
    pub selector: String,
}

#[derive(Debug, Deserialize)]
pub struct MultipageSpiderSettings {
    pub name: String,
    pub base_url: String,
    pub subroutes: Vec<String>,
    pub selector: String,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory.");
    let configuration_directory = base_path.join("configuration");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let settings = Config::builder()
        // Read the "default" configuration file
        .add_source(config::File::from(configuration_directory.join("base")).required(true))
        // Layer on the environment-specific values.
        .add_source(
            config::File::from(configuration_directory.join(environment.as_str())).required(true),
        )
        // Add in settings from environment variables (with a prefix of APP and '__' as separator)
        // E.g. `APP_APPLICATION__PORT=5001` would set `Settings.application.port`
        .add_source(config::Environment::with_prefix("app").separator("__"))
        .build()?;

    settings.try_deserialize()
}

/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
