use std::{future::pending, io, num::NonZeroUsize};

use jsonrpsee::server::ServerHandle;
use metrics_exporter_prometheus::PrometheusBuilder;
use simulation_tasks::SimulationTaskSet;
use tracing_subscriber::fmt::format::FmtSpan;

use crate::{
    app_config::{AppConfig, LogFormat, MetricsConfig, SimulationConfig, TracingConfig},
    metrics::{MetricsServer, start_metrics_server},
    rpc_server,
};

pub async fn run(config: AppConfig) -> io::Result<()> {
    init_tracing(&config.tracing)?;

    let simulation_tasks = create_simulation_task_set(&config.simulation)?;
    let mut metrics_server = start_metrics_server_if_enabled(&config.metrics).await?;
    let rpc_handle = rpc_server::start(&config, simulation_tasks).await?;

    wait_for_listener_stop(rpc_handle, &mut metrics_server).await
}

fn init_tracing(config: &TracingConfig) -> io::Result<()> {
    let level: tracing::Level = config
        .level
        .parse()
        .map_err(|_| configuration_error(format!("invalid tracing level: {}", config.level)))?;
    let subscriber_builder = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true);

    match config.format {
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

    Ok(())
}

fn create_simulation_task_set(config: &SimulationConfig) -> io::Result<SimulationTaskSet> {
    let max_concurrent = NonZeroUsize::new(config.max_concurrent).ok_or_else(|| {
        configuration_error("simulation.max_concurrent must be greater than zero")
    })?;

    Ok(SimulationTaskSet::new(max_concurrent))
}

async fn start_metrics_server_if_enabled(
    config: &MetricsConfig,
) -> io::Result<Option<MetricsServer>> {
    if !config.enabled {
        return Ok(None);
    }

    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|error| {
            io::Error::other(format!("failed to install Prometheus recorder: {error}"))
        })?;
    let address = config
        .listen_address
        .parse()
        .map_err(|error| configuration_error(format!("invalid metrics address: {error}")))?;
    let metrics_server = start_metrics_server(address, prometheus_handle)
        .await
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("failed to start metrics server: {error}"),
            )
        })?;

    Ok(Some(metrics_server))
}

async fn wait_for_listener_stop(
    rpc_handle: ServerHandle,
    metrics_server: &mut Option<MetricsServer>,
) -> io::Result<()> {
    tokio::select! {
        _ = rpc_handle.stopped() => Err(io::Error::other("RPC server stopped unexpectedly")),
        result = wait_for_metrics_server(metrics_server) => match result {
            Ok(()) => Err(io::Error::other("metrics server stopped unexpectedly")),
            Err(error) => Err(error),
        },
    }
}

async fn wait_for_metrics_server(metrics_server: &mut Option<MetricsServer>) -> io::Result<()> {
    match metrics_server {
        Some(server) => server.wait().await,
        None => pending().await,
    }
}

fn configuration_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::create_simulation_task_set;
    use crate::app_config::SimulationConfig;

    #[test]
    fn simulation_task_set_rejects_zero_capacity() {
        let error = create_simulation_task_set(&SimulationConfig { max_concurrent: 0 })
            .expect_err("zero simulation capacity must be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            error.to_string(),
            "simulation.max_concurrent must be greater than zero"
        );
    }
}
