use std::error::Error;
use std::sync::Arc;

use evm_engine::EvmEngine;
use jsonrpsee::server::Server;
use metrics_exporter_prometheus::PrometheusBuilder;
use rpc_server::{DryrunRpcServer, RpcHandler};
use simulation_service::SimulationService;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::app_config::{AppConfig, LogFormat};
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

    if app_config.metrics.enabled {
        let builder = PrometheusBuilder::new();
        let prometheus_handle = builder
            .install_recorder()
            .expect("Failed to install Prometheus recorder");

        let metrics_addr: std::net::SocketAddr = app_config
            .metrics
            .listen_address
            .parse()
            .map_err(|e| format!("Invalid metrics address: {}", e))?;

        let _metrics_handle = start_metrics_server(metrics_addr, prometheus_handle)
            .await
            .map_err(|e| format!("Failed to start metrics server: {}", e))?;
    }

    let server = Server::builder()
        .build(format!(
            "{}:{}",
            app_config.server.host, app_config.server.port
        ))
        .await?;

    let addr = server.local_addr()?;

    let evm_engine = Arc::new(EvmEngine::new(app_config.ethereum.rpc_url));
    let simulation_service = Arc::new(SimulationService::new(evm_engine));
    let rpc_handler = RpcHandler::new(simulation_service);
    let handle = server.start(rpc_handler.into_rpc());

    info!("RPC server started at {}", addr);

    handle.stopped().await;

    Ok(())
}
