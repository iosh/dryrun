use std::{error::Error, sync::Arc};

use configs::AppConfig;
use jsonrpsee::server::Server;
use rpc_server::{RpcHandler, SimulationRpcServer};
use simulation_core::SimulationService;
use tracing::info;
use tracing_subscriber::fmt::format::FmtSpan;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let configs = AppConfig::new()?;

    let level = configs
        .tracing
        .level
        .parse()
        .map_err(|_| {
            eprintln!("Invalid tracing level: {}", configs.tracing.level);
            configs.tracing.level.clone()
        })
        .unwrap_or(tracing::Level::INFO);

    let subscriber_builder = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true);

    match configs.tracing.format {
        configs::LogFormat::Pretty => {
            subscriber_builder
                .with_span_events(FmtSpan::CLOSE)
                .pretty()
                .init();
        }
        configs::LogFormat::Json => {
            subscriber_builder
                .with_span_events(FmtSpan::CLOSE)
                .json()
                .init();
        }
    }

    let server = Server::builder()
        .build(format!("{}:{}", configs.server.host, configs.server.port))
        .await?;

    let addr = server.local_addr()?;

    let service = Arc::new(SimulationService::new(configs.evm));
    let handle = server.start(RpcHandler::new(service).into_rpc());

    info!("RPC server started at {}", addr);

    handle.stopped().await;

    Ok(())
}
