use std::error::Error;

use app_config::AppConfig;

mod app;
mod app_config;
mod metrics;
mod rpc;
mod rpc_server;
mod simulation_tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    app::run(AppConfig::load()?).await?;

    Ok(())
}
