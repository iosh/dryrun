use std::{env, error::Error, net::SocketAddr};

use jsonrpsee::{RpcModule, server::Server};
use serde_json::json;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8547";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let addr = listen_addr()?;
    let server = Server::builder().build(addr).await?;
    let local_addr = server.local_addr()?;

    let mut module = RpcModule::new(());
    module.register_method("dryrun_conflux_health", |_, _, _| {
        json!({
            "service": "dryrun-conflux",
            "status": "ok"
        })
    })?;

    let handle = server.start(module);

    info!("dryrun-conflux RPC server started at {}", local_addr);

    handle.stopped().await;

    Ok(())
}

fn listen_addr() -> Result<SocketAddr, Box<dyn Error>> {
    let value =
        env::var("DRYRUN_CONFLUX_LISTEN_ADDR").unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.to_owned());

    Ok(value.parse()?)
}
