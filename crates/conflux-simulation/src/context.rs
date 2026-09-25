use cfx_executor::spec::CommonParams;
use cfx_types::{Address, H256, SpaceMap, U256};
use primitives::BlockNumber;
use thiserror::Error;

use crate::state::{CoreSpaceRpcBlock, EspaceRpcBlock};

// Pre-PoS pivots have no consensus reference. Ordinary eSpace execution does
// not depend on these facts either.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ConsensusContext {
    pub(crate) pos_view: Option<u64>,
    pub(crate) finalized_epoch: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct PivotBlock {
    pub(crate) number: BlockNumber,
    pub(crate) epoch_number: u64,
    pub(crate) author: Address,
    pub(crate) timestamp: u64,
    pub(crate) hash: H256,
    pub(crate) base_fee_per_gas: Option<U256>,
}

impl TryFrom<&CoreSpaceRpcBlock> for PivotBlock {
    type Error = ConfluxBlockContextError;

    fn try_from(block: &CoreSpaceRpcBlock) -> Result<Self, Self::Error> {
        Ok(Self {
            number: required_pivot_number(block.block_number, "blockNumber")?,
            epoch_number: required_pivot_number(block.epoch_number, "epochNumber")?,
            author: block.miner,
            timestamp: u256_to_u64(block.timestamp, "timestamp")?,
            hash: block.hash,
            base_fee_per_gas: block.base_fee_per_gas,
        })
    }
}

/// Block facts prepared once, before transaction completion or VM execution.
#[derive(Debug, Clone)]
pub(crate) struct ExecutionBlockContext {
    pub(crate) number: BlockNumber,
    pub(crate) epoch_height: u64,
    pub(crate) author: Address,
    pub(crate) timestamp: u64,
    pub(crate) epoch_hash: H256,
    pub(crate) consensus: ConsensusContext,
    pub(crate) base_gas_price: SpaceMap<U256>,
}

impl ExecutionBlockContext {
    pub(crate) fn from_pivot(
        pivot: &PivotBlock,
        espace: &EspaceRpcBlock,
        consensus: ConsensusContext,
        params: &CommonParams,
    ) -> Result<Self, ConfluxBlockContextError> {
        // State reads stay at the selected pivot. Execution and fork rules use
        // the next block and epoch, matching Conflux block assembly.
        let number = next_height(pivot.number, "blockNumber")?;
        let epoch_height = next_height(pivot.epoch_number, "epochNumber")?;
        let activation = params.transition_heights.cip1559;
        let base_gas_price = if epoch_height < activation {
            SpaceMap::new(U256::zero(), U256::zero())
        } else if epoch_height == activation {
            params.init_base_price()
        } else {
            SpaceMap::new(
                pivot
                    .base_fee_per_gas
                    .ok_or(ConfluxBlockContextError::MissingBaseFee {
                        space: "Core Space",
                        execution_epoch_height: epoch_height,
                    })?,
                espace
                    .base_fee_per_gas
                    .ok_or(ConfluxBlockContextError::MissingBaseFee {
                        space: "eSpace",
                        execution_epoch_height: epoch_height,
                    })?,
            )
        };

        Ok(Self {
            number,
            epoch_height,
            author: pivot.author,
            timestamp: pivot.timestamp,
            epoch_hash: pivot.hash,
            consensus,
            base_gas_price,
        })
    }
}

/// Block data cannot provide the context required for Conflux simulation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfluxBlockContextError {
    #[error("Core Space pivot block is missing {field}")]
    MissingField { field: &'static str },
    #[error("Core Space pivot block {field} exceeds u64: {value}")]
    ValueOutOfRange { field: &'static str, value: U256 },
    #[error("The next execution {field} exceeds u64")]
    HeightOverflow { field: &'static str },
    #[error(
        "{space} block is missing baseFeePerGas required to simulate epoch {execution_epoch_height}"
    )]
    MissingBaseFee {
        space: &'static str,
        execution_epoch_height: u64,
    },
}

fn required_pivot_number(
    value: Option<U256>,
    field: &'static str,
) -> Result<u64, ConfluxBlockContextError> {
    u256_to_u64(
        value.ok_or(ConfluxBlockContextError::MissingField { field })?,
        field,
    )
}

fn u256_to_u64(value: U256, field: &'static str) -> Result<u64, ConfluxBlockContextError> {
    u64::try_from(value).map_err(|_| ConfluxBlockContextError::ValueOutOfRange { field, value })
}

fn next_height(value: u64, field: &'static str) -> Result<u64, ConfluxBlockContextError> {
    value
        .checked_add(1)
        .ok_or(ConfluxBlockContextError::HeightOverflow { field })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain_spec::ConfluxChainSpec;
    use cfx_types::Space;

    #[test]
    fn fees_follow_the_execution_epoch_at_cip1559_activation() {
        let chain = ConfluxChainSpec::mainnet();
        let activation = chain.common_params().transition_heights.cip1559;
        let mut pivot = PivotBlock {
            number: 240_000_000,
            epoch_number: activation - 2,
            author: Address::zero(),
            timestamp: 1_700_000_000,
            hash: H256::zero(),
            base_fee_per_gas: Some(U256::from(2_000_000_000_u64)),
        };
        let mut espace = EspaceRpcBlock {
            hash: Default::default(),
            number: activation - 2,
            base_fee_per_gas: Some(U256::from(30_000_000_000_u64)),
        };
        // Mainnet CIP-1559 starts at 1 Gdrip in Core and 20 Gdrip in eSpace.
        for (epoch, core_fee, espace_fee) in [
            (activation - 2, 0_u64, 0_u64),
            (activation - 1, 1_000_000_000, 20_000_000_000),
            (activation, 2_000_000_000, 30_000_000_000),
        ] {
            pivot.epoch_number = epoch;
            espace.number = epoch;
            let context = ExecutionBlockContext::from_pivot(
                &pivot,
                &espace,
                ConsensusContext::default(),
                chain.common_params(),
            )
            .unwrap();
            assert_eq!(context.base_gas_price[Space::Native], U256::from(core_fee));
            assert_eq!(
                context.base_gas_price[Space::Ethereum],
                U256::from(espace_fee)
            );
        }

        for (core_fee, espace_fee) in [(None, Some(U256::one())), (Some(U256::one()), None)] {
            pivot.base_fee_per_gas = core_fee;
            espace.base_fee_per_gas = espace_fee;
            assert!(matches!(
                ExecutionBlockContext::from_pivot(
                    &pivot,
                    &espace,
                    ConsensusContext::default(),
                    chain.common_params(),
                ),
                Err(ConfluxBlockContextError::MissingBaseFee { .. })
            ));
        }
    }
}
