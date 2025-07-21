use std::{error::Error, sync::Arc};

use configs::AppConfig;
use jsonrpsee::server::Server;
use rpc_server::{RpcHandler, SimulationRpcServer};
use simulation_core::SimulationService;
use tracing::info;

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
            subscriber_builder.pretty().init();
        }
        configs::LogFormat::Json => {
            subscriber_builder.json().init();
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
