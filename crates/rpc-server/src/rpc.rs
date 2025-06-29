use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[rpc(server)]
pub trait SimulationRpc {
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;
}
