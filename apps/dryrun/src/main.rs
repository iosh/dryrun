use std::{error::Error, io, time::Duration};

use tracing::{error, info};

mod config;
mod logging;
mod metrics;
mod rpc;
mod tasks;

use crate::{config::AppConfig, metrics::MetricsServer, tasks::SimulationTaskSet};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = AppConfig::load()?;
    logging::init(&config.tracing)?;
    run(config).await?;
    Ok(())
}

async fn run(config: AppConfig) -> io::Result<()> {
    let simulation_tasks = SimulationTaskSet::new(
        config.simulation.max_concurrent,
        Duration::from_secs(config.simulation.response_timeout_seconds),
    )?;
    let mut metrics_server = MetricsServer::start(&config.metrics).await?;
    let rpc_handle = match rpc::start(&config, simulation_tasks.clone()).await {
        Ok(handle) => handle,
        Err(error) => {
            if let Err(cleanup_error) = metrics_server.shutdown().await {
                error!(error = ?cleanup_error, "metrics server cleanup failed after RPC startup failure");
            }
            return Err(error);
        }
    };

    let run_result = tokio::select! {
        result = shutdown_signal() => result,
        _ = rpc_handle.clone().stopped() => {
            Err(io::Error::other("RPC server stopped unexpectedly"))
        }
        result = metrics_server.stopped() => {
            Err(match result {
                Ok(()) => io::Error::other("metrics server stopped unexpectedly"),
                Err(error) => error,
            })
        }
    };

    if run_result.is_ok() {
        info!("shutdown signal received");
    }

    let _ = rpc_handle.stop();
    simulation_tasks.close();

    let (_, cleanup_result) = tokio::join!(rpc_handle.stopped(), metrics_server.shutdown());
    simulation_tasks.drain().await;

    match (run_result, cleanup_result) {
        (Ok(()), Ok(())) => {
            info!("shutdown complete");
            Ok(())
        }
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup_error)) => {
            error!(
                error = ?cleanup_error,
                "shutdown cleanup failed after the primary runtime error"
            );
            Err(error)
        }
    }
}

async fn shutdown_signal() -> io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}
