use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::interface::{EvmSimulateTransactionRequest, EvmSimulateTransactionResponse};

#[rpc(server)]
pub trait DryrunRpc {
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;

    #[method(name = "dryrun_evm_simulateTransaction")]
    async fn dryrun_evm_simulate_transaction(
        &self,
        request: EvmSimulateTransactionRequest,
    ) -> RpcResult<EvmSimulateTransactionResponse>;
}
