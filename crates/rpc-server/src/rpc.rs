use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::interface::{BlockRef, EvmSimulateTransactionResponse, Transaction};

#[rpc(server)]
pub trait DryrunRpc {
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;

    #[method(name = "dryrun_evm_simulateTransaction")]
    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: Transaction,
        block: Option<BlockRef>,
    ) -> RpcResult<EvmSimulateTransactionResponse>;
}
