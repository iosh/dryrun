use std::{future::pending, io, net::SocketAddr};

use axum::{Router, routing::get};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tokio::task::JoinHandle;
use tracing::info;

use crate::config::MetricsConfig;

/// Owns the optional metrics endpoint and its background task.
pub(crate) struct MetricsServer {
    task: Option<JoinHandle<io::Result<()>>>,
}

impl MetricsServer {
    pub(crate) async fn start(config: &MetricsConfig) -> io::Result<Self> {
        if !config.enabled {
            return Ok(Self { task: None });
        }

        let address = config.listen_address.parse().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid metrics address: {error}"),
            )
        })?;
        let handle = PrometheusBuilder::new()
            .install_recorder()
            .map_err(|error| {
                io::Error::other(format!("failed to install Prometheus recorder: {error}"))
            })?;
        Self::bind(address, handle).await.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to start metrics server: {error}"),
            )
        })
    }

    async fn bind(address: SocketAddr, handle: PrometheusHandle) -> io::Result<Self> {
        let app = Router::new().route(
            "/metrics",
            get(move || {
                let handle = handle.clone();
                async move { handle.render() }
            }),
        );
        let listener = tokio::net::TcpListener::bind(address).await?;
        let local_addr = listener.local_addr()?;
        let task = tokio::spawn(async move { axum::serve(listener, app).await });
        info!("metrics server started at http://{local_addr}/metrics");

        Ok(Self { task: Some(task) })
    }

    /// Waits for the server to exit. Disabled metrics never trigger shutdown.
    pub(crate) async fn stopped(&mut self) -> io::Result<()> {
        let Some(task) = self.task.as_mut() else {
            return pending().await;
        };
        task.await.map_err(io::Error::other)?
    }
}
