
use jsonrpsee::server::{Server};
#[tokio::main]
async fn main() {
    let server = Server::builder().build("127.0.0.1:8080").await?;
    let addr = server.local_addr()?;

    let handle = server.start(rpc::RpcServer{});

    tokio::select! {
        _ = handle => {
            println!("Server started at {}", addr);
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down server...");
        }
        _ = server.stop() => {
            println!("Server stopped");
        }
    }
    Ok(addr)
}