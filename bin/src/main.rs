use std::{error::Error, sync::Arc};

use configs::AppConfig;
use jsonrpsee::server::Server;
use rpc_server::{RpcHandler, SimulationRpcServer};
use simulation_core::SimulationService;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();

    tracing::subscriber::set_global_default(subscriber)?;

    let configs = AppConfig::new()?;

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
