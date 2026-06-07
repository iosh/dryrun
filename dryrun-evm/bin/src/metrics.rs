use axum::{Router, http::StatusCode, routing::get};
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::SocketAddr;
use tracing::{error, info};

async fn metrics_handler(handle: PrometheusHandle) -> Result<String, StatusCode> {
    Ok(handle.render())
}

async fn health_handler() -> &'static str {
    "OK"
}

pub async fn start_metrics_server(
    addr: SocketAddr,
    prometheus_handle: PrometheusHandle,
) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
    let app = Router::new()
        .route(
            "/metrics",
            get(move || metrics_handler(prometheus_handle.clone())),
        )
        .route("/health", get(health_handler));

    info!("Starting metrics server at http://{}/metrics", addr);
    info!("Health check available at http://{}/health", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind metrics server to {}: {}", addr, e))?;

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Metrics server error: {}", e);
        }
    });
    Ok(handle)
}
