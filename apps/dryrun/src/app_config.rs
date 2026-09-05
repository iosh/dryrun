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

#[derive(Debug, Deserialize)]
pub struct EthereumSimulationLimitsConfig {
    #[serde(default = "default_max_occurrence_checkpoints")]
    pub max_occurrence_checkpoints: usize,
    #[serde(default = "default_max_retained_state_entries")]
    pub max_retained_state_entries: usize,
    #[serde(default = "default_max_state_reads")]
    pub max_state_reads: usize,
    #[serde(default = "default_max_read_calls")]
    pub max_read_calls: usize,
    #[serde(default = "default_read_call_gas_limit")]
    pub read_call_gas_limit: u64,
    #[serde(default = "default_max_read_call_output_bytes")]
    pub max_read_call_output_bytes: usize,
}

impl Default for EthereumSimulationLimitsConfig {
    fn default() -> Self {
        Self {
            max_occurrence_checkpoints: default_max_occurrence_checkpoints(),
            max_retained_state_entries: default_max_retained_state_entries(),
            max_state_reads: default_max_state_reads(),
            max_read_calls: default_max_read_calls(),
            read_call_gas_limit: default_read_call_gas_limit(),
            max_read_call_output_bytes: default_max_read_call_output_bytes(),
        }
    }
}

fn default_max_occurrence_checkpoints() -> usize {
    128
}

fn default_max_retained_state_entries() -> usize {
    100_000
}

fn default_max_state_reads() -> usize {
    1_024
}

fn default_max_read_calls() -> usize {
    64
}

fn default_read_call_gas_limit() -> u64 {
    5_000_000
}

fn default_max_read_call_output_bytes() -> usize {
    256 * 1024
}

#[derive(Debug, Deserialize)]
pub struct ConfluxConfig {
    pub espace_rpc_url: String,
    pub core_space_rpc_url: String,
    #[serde(default)]
    pub espace_limits: EspaceSimulationLimitsConfig,
}

#[derive(Debug, Deserialize)]
pub struct EspaceSimulationLimitsConfig {
    #[serde(default = "default_max_state_reads")]
    pub max_state_reads: usize,
    #[serde(default = "default_max_read_calls")]
    pub max_read_calls: usize,
    #[serde(default = "default_read_call_gas_limit")]
    pub read_call_gas_limit: u64,
    #[serde(default = "default_max_read_call_output_bytes")]
    pub max_read_call_output_bytes: usize,
}

impl Default for EspaceSimulationLimitsConfig {
    fn default() -> Self {
        Self {
            max_state_reads: default_max_state_reads(),
            max_read_calls: default_max_read_calls(),
            read_call_gas_limit: default_read_call_gas_limit(),
            max_read_call_output_bytes: default_max_read_call_output_bytes(),
        }
    }
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
