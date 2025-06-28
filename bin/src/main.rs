use std::error::Error;

use jsonrpsee::server::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();

    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
