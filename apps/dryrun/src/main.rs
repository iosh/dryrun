use std::error::Error;

use app_config::AppConfig;

mod app;
mod app_config;
mod metrics;
mod rpc_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    app::run(AppConfig::load()?).await?;

    Ok(())
}
