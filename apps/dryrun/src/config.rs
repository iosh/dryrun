use ::config::{Config, ConfigError, Environment, File};
use alloy::transports::http::reqwest::Url;
use serde::Deserialize;
use simulation_core::Limits;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub ethereum: EthereumConfig,
    pub conflux: Option<ConfluxConfig>,
    pub simulation: SimulationConfig,
    pub tracing: TracingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Deserialize)]
pub struct EthereumConfig {
    pub rpc_url: Url,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Deserialize)]
pub struct ConfluxConfig {
    pub core_rpc_url: Url,
    pub espace_rpc_url: Url,
    #[serde(default)]
    pub limits: Limits,
}

#[derive(Debug, Deserialize)]
pub struct SimulationConfig {
    pub max_concurrent: usize,
    #[serde(default = "default_response_timeout_seconds")]
    pub response_timeout_seconds: u64,
}

fn default_response_timeout_seconds() -> u64 {
    120
}

#[derive(Debug, Deserialize)]
pub struct TracingConfig {
    pub level: String,
    pub format: LogFormat,
}

#[derive(Clone, Copy, Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub listen_address: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::with_name("env").required(false))
            .add_source(File::with_name("local").required(false))
            .add_source(
                Environment::with_prefix("app")
                    .prefix_separator("_")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        config.try_deserialize()
    }
}
