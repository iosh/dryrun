use std::env;

use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub evm: EvmConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct EvmConfig {
    pub rpc_url: String,
}

impl AppConfig {
    pub fn new() -> Result<Self, config::ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            .add_source(File::with_name(&run_mode).required(false))
            .add_source(File::with_name("local").required(false))
            .add_source(Environment::with_prefix("app"))
            .build()?;

        s.try_deserialize()
    }
}
