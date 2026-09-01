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
    #[serde(default)]
    pub limits: EthereumSimulationLimitsConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct EthereumSimulationLimitsConfig {
    pub max_occurrence_checkpoints: Option<usize>,
    pub max_retained_state_entries: Option<usize>,
    pub max_state_reads: Option<usize>,
    pub max_read_calls: Option<usize>,
    pub read_call_gas_limit: Option<u64>,
    pub max_read_call_output_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ConfluxConfig {
    pub espace_rpc_url: String,
    pub core_space_rpc_url: String,
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
