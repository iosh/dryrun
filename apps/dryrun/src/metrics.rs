use std::{io, net::SocketAddr};

use axum::{Router, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use tokio::sync::oneshot;
use tracing::info;

pub struct MetricsServer {
    stopped: oneshot::Receiver<io::Result<()>>,
}

async fn metrics_handler(handle: PrometheusHandle) -> String {
    handle.render()
}

pub async fn start_metrics_server(
    addr: SocketAddr,
    prometheus_handle: PrometheusHandle,
) -> io::Result<MetricsServer> {
    let app = Router::new().route(
        "/metrics",
        get(move || metrics_handler(prometheus_handle.clone())),
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;
    let (stopped_tx, stopped_rx) = oneshot::channel();

    tokio::spawn(async move {
        let result = axum::serve(listener, app).await;
        let _ = stopped_tx.send(result);
    });

    info!("metrics server started at http://{local_addr}/metrics");

    Ok(MetricsServer {
        stopped: stopped_rx,
    })
}

impl MetricsServer {
    pub async fn wait(&mut self) -> io::Result<()> {
        match (&mut self.stopped).await {
            Ok(result) => result,
            Err(_) => Err(io::Error::other(
                "metrics server task exited without reporting its result",
            )),
        }
    }
}
