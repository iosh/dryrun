mod error;
mod types;

use std::sync::Arc;

use evm_engine::{EvmEngine, EvmExecution, EvmExecutionInput, EvmSimulation};

pub use error::SimulationServiceError;
pub use types::{
    AccessListItem, ApprovalChange, ApprovalForAllChange, Asset, BlockRef, BurnChange, Change,
    Collection, EvmTransaction, EvmTransactionType, MintChange, SimulateEvmTransactionInput,
    SimulateEvmTransactionOutput, SimulatedBlock, SimulationError, SimulationStatus,
    TransferChange,
};

#[derive(Debug, Clone)]
pub struct SimulationService {
    evm_engine: Arc<EvmEngine>,
}

impl SimulationService {
    pub fn new(evm_engine: Arc<EvmEngine>) -> Self {
        Self { evm_engine }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        let SimulateEvmTransactionInput { block, transaction } = input;
        let simulation = self
            .evm_engine
            .simulate(EvmExecutionInput { block, transaction })
            .await?;

        Ok(SimulateEvmTransactionOutput::from_engine_simulation(
            simulation,
        ))
    }
}

impl SimulateEvmTransactionOutput {
    fn from_engine_simulation(simulation: EvmSimulation) -> Self {
        let (
            EvmExecution {
                chain_id,
                block,
                status,
                gas_used,
                gas_limit,
                output,
                failure: error,
            },
            changes,
        ) = simulation.into_parts();

        Self {
            chain_id,
            block,
            status,
            gas_used,
            gas_limit,
            output,
            error,
            changes,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};
    use evm_engine::{EvmExecution, EvmSimulation};

    use super::*;

    #[test]
    fn engine_simulation_maps_into_service_output() {
        let simulation = EvmSimulation::new(
            EvmExecution {
                chain_id: 1,
                block: evm_engine::SimulatedBlock {
                    number: 0x1234,
                    hash: B256::from_str(
                        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("hash"),
                },
                status: evm_engine::EvmExecutionStatus::Success,
                gas_used: 0x5208,
                gas_limit: 0x5300,
                output: Bytes::from_str("0x0102").expect("bytes"),
                failure: None,
            },
            vec![evm_engine::Change::Transfer(evm_engine::TransferChange {
                asset: evm_engine::Asset::Erc20 {
                    contract_address: Address::from_str(
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    )
                    .expect("token"),
                    symbol: Some("USDC".to_string()),
                    decimals: Some(6),
                    name: None,
                },
                from: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("from"),
                to: Address::from_str("0x2222222222222222222222222222222222222222").expect("to"),
                amount: Some(U256::from(0x1234_u64)),
            })],
        );

        let mapped = SimulateEvmTransactionOutput::from_engine_simulation(simulation);
        assert_eq!(mapped.changes.len(), 1);

        let Change::Transfer(change) = &mapped.changes[0] else {
            panic!("expected transfer change");
        };

        assert_eq!(change.amount, Some(U256::from(0x1234_u64)));

        let Asset::Erc20 {
            symbol, decimals, ..
        } = &change.asset
        else {
            panic!("expected erc20 asset");
        };

        assert_eq!(symbol.as_deref(), Some("USDC"));
        assert_eq!(*decimals, Some(6));
    }
}
