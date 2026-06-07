use std::{io, net::SocketAddr};

use axum::{Router, routing::get};
use tokio::task::JoinHandle;
use tracing::{error, info};

async fn health_handler() -> &'static str {
    "OK"
}

pub async fn start_health_server(addr: SocketAddr) -> io::Result<JoinHandle<()>> {
    let app = Router::new().route("/health", get(health_handler));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    info!("dryrun-conflux health check available at http://{local_addr}/health");

    let handle = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            error!("dryrun-conflux health server error: {error}");
        }
    });

    Ok(handle)
}
