use std::error::Error;

use jsonrpsee::server::Server;
use rpc_server::{RpcHandler, SimulationRpcServer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();

    tracing::subscriber::set_global_default(subscriber)?;

    let server = Server::builder().build("127.0.0.1:8000").await?;

    let addr = server.local_addr()?;

    let handle = server.start(RpcHandler::new().into_rpc());

    info!("RPC server started at {}", addr);

    handle.stopped().await;

    Ok(())
}
