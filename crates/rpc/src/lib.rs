use std::fmt::Result;

use jsonrpsee::proc_macros::rpc;
#[rpc(server)]
pub trait Rpc {
    #[method(name = "dry_run_emv_simulateTransaction")]
    async fn dry_run_emv_simulate_transaction(&self) -> Result<String, jsonrpsee::core::Error>;
}

pub struct RpcServer {}


impl Rpc for RpcServer {
    async fn dry_run_emv_simulate_transaction(&self) -> Result<String, jsonrpsee::core::Error> {
        Ok("Transaction simulated".into())
    }
}
