use std::{error::Error, io, num::NonZeroUsize, sync::Arc, time::Duration};

use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use evm_engine::EvmEngine;
use evm_rpc::{DryrunRpcServer, RpcHandler};
use evm_service::SimulationService;
use jsonrpsee::server::Server;
use metrics_exporter_prometheus::PrometheusBuilder;
use simulation_tasks::SimulationTaskSet;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::app_config::{AppConfig, EthereumConfig, LogFormat, SimulationConfig};
use crate::metrics::start_metrics_server;

mod app_config;
mod metrics;
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app_config = AppConfig::load()?;

    let level = app_config
        .tracing
        .level
        .parse()
        .map_err(|_| {
            eprintln!("Invalid tracing level: {}", app_config.tracing.level);
            app_config.tracing.level.clone()
        })
        .unwrap_or(tracing::Level::INFO);

    let subscriber_builder = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true);

    match app_config.tracing.format {
        LogFormat::Pretty => {
            subscriber_builder
                .with_span_events(FmtSpan::CLOSE)
                .pretty()
                .init();
        }
        LogFormat::Json => {
            subscriber_builder
                .with_span_events(FmtSpan::CLOSE)
                .json()
                .init();
        }
    }

    let simulation_tasks = create_simulation_task_set(&app_config.simulation)?;
    let ethereum_provider = create_ethereum_provider(&app_config.ethereum).await?;
    let evm_engine = Arc::new(EvmEngine::new(
        ethereum_provider.clone(),
        tokio::runtime::Handle::current(),
    ));
    let simulation_service = Arc::new(SimulationService::new(
        ethereum_provider,
        evm_engine,
        simulation_tasks.clone(),
    ));

    let _metrics_handle = if app_config.metrics.enabled {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder.install_recorder().map_err(|error| {
            startup_error(format!("failed to install Prometheus recorder: {error}"))
        })?;

        let metrics_addr: std::net::SocketAddr = app_config
            .metrics
            .listen_address
            .parse()
            .map_err(|error| startup_error(format!("invalid metrics address: {error}")))?;

        Some(
            start_metrics_server(metrics_addr, prometheus_handle)
                .await
                .map_err(|error| {
                    startup_error(format!("failed to start metrics server: {error}"))
                })?,
        )
    } else {
        None
    };

    let server = Server::builder()
        .build(format!(
            "{}:{}",
            app_config.server.host, app_config.server.port
        ))
        .await?;

    let addr = server.local_addr()?;

    let rpc_handler = RpcHandler::new(simulation_service);
    let handle = server.start(rpc_handler.into_rpc());

    info!("RPC server started at {}", addr);

    handle.stopped().await;
    simulation_tasks.close();
    simulation_tasks.wait().await;

    Ok(())
}

fn create_simulation_task_set(config: &SimulationConfig) -> Result<SimulationTaskSet, io::Error> {
    let max_concurrent = NonZeroUsize::new(config.max_concurrent)
        .ok_or_else(|| startup_error("simulation.max_concurrent must be greater than zero"))?;

    Ok(SimulationTaskSet::new(
        max_concurrent,
        Duration::from_secs(config.admission_timeout_seconds),
    ))
}

async fn create_ethereum_provider(config: &EthereumConfig) -> Result<DynProvider, Box<dyn Error>> {
    if config.request_timeout_seconds == 0 {
        return Err(
            startup_error("ethereum.request_timeout_seconds must be greater than zero").into(),
        );
    }

    let rpc_url: reqwest::Url = config
        .rpc_url
        .parse()
        .map_err(|error| startup_error(format!("invalid Ethereum RPC URL: {error}")))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.request_timeout_seconds))
        .build()
        .map_err(|error| {
            startup_error(format!("failed to create Ethereum HTTP client: {error}"))
        })?;
    let provider = ProviderBuilder::new()
        .connect_reqwest(client, rpc_url)
        .erased();

    Ok(provider)
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use std::io;

    use crate::{app_config::SimulationConfig, create_simulation_task_set};

    #[test]
    fn simulation_task_set_rejects_zero_capacity() {
        let error = create_simulation_task_set(&SimulationConfig {
            max_concurrent: 0,
            admission_timeout_seconds: 1,
        })
        .expect_err("zero simulation capacity must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "simulation.max_concurrent must be greater than zero"
        );
    }
}
