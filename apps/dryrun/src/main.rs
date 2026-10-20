use std::{error::Error, io, time::Duration};

use tracing::info;

mod config;
mod logging;
mod metrics;
mod rpc;
mod tasks;

use crate::{config::AppConfig, metrics::MetricsServer, tasks::SimulationTaskSet};

fn main() -> Result<(), Box<dyn Error>> {
    let config = AppConfig::load()?;
    logging::init(&config.tracing)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(run(config));
    // Process shutdown does not wait for VM work on blocking threads.
    runtime.shutdown_background();
    result?;
    Ok(())
}

async fn run(config: AppConfig) -> io::Result<()> {
    let simulation_tasks = SimulationTaskSet::new(
        config.simulation.max_concurrent,
        Duration::from_secs(config.simulation.response_timeout_seconds),
    )?;
    let mut metrics_server = MetricsServer::start(&config.metrics).await?;
    let rpc_handle = rpc::start(&config, simulation_tasks.clone()).await?;

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

    run_result
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
