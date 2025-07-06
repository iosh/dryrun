use alloy::rpc::types::{BlockId, BlockOverrides, TransactionRequest, state::StateOverride};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use types::EvmSimulateOutput;

#[rpc(server)]
pub trait SimulationRpc {
    #[method(name = "health")]
    async fn health(&self) -> RpcResult<String>;

    #[method(name = "dryrun_evm_simulate_transaction")]
    async fn dryrun_evm_simulate_transaction(
        &self,
        transaction: TransactionRequest,
        block_id: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<BlockOverrides>,
    ) -> RpcResult<EvmSimulateOutput>;
}
