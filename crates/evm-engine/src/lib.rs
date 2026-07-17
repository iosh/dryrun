mod chain_spec;
mod change;
mod change_detection;
mod change_observation;
mod error;
mod execution;
mod simulation;
mod transaction;

pub use change::{
    ApprovalChange, ApprovalForAllChange, Asset, BurnChange, Change, Collection, Erc20AssetDisplay,
    Erc721CollectionDisplay, Erc1155CollectionDisplay, MintChange, NativeAssetDisplay,
    NftTokenDisplay, TransferChange,
};
pub use error::{EvmEngineError, EvmEngineInternalKind};
use execution::simulate_execution;
pub use simulation::{
    EvmExecution, EvmExecutionFailure, EvmExecutionFailureCode, EvmExecutionStatus, EvmSimulation,
    SimulatedBlock,
};
pub use transaction::{
    AccessListItem, BlockRef, EvmExecutionInput, EvmTransaction, EvmTransactionVariant,
};

#[derive(Debug, Clone)]
pub struct EvmEngine {
    rpc_url: String,
}

impl EvmEngine {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub async fn simulate(
        &self,
        input: EvmExecutionInput,
    ) -> Result<EvmSimulation, EvmEngineError> {
        simulate_execution(&self.rpc_url, input).await
    }
}
