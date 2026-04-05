use jsonrpsee::{core::RpcResult, proc_macros::rpc};

use crate::interface::{
    BlockRef, EvmSimulateTransactionResponse, SimulateTransactionOptions, Transaction,
};

#[rpc(server)]
pub trait DryrunRpc {
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;

    #[method(name = "dryrun_evm_simulateTransaction", param_kind = map)]
    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: Transaction,
        block: Option<BlockRef>,
        options: Option<SimulateTransactionOptions>,
    ) -> RpcResult<EvmSimulateTransactionResponse>;
}
