use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub ethereum: EthereumConfig,
    pub conflux: ConfluxConfig,
    pub simulation: SimulationConfig,
    pub tracing: TracingConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Deserialize)]
pub struct EthereumConfig {
    pub rpc_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfluxConfig {
    pub espace_rpc_url: String,
    pub core_space_rpc_url: String,
}

#[derive(Debug, Deserialize)]
pub struct SimulationConfig {
    pub max_concurrent: usize,
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
    pub fn load() -> Result<Self, config::ConfigError> {
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
