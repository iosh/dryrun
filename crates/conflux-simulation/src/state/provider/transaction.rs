use crate::primitive::alloy_u256_from_u64;
use alloy::{
    eips::BlockId,
    primitives::{Address as AlloyAddress, U256 as AlloyU256},
    providers::Provider,
    rpc::client::NoParams,
};
use cfx_types::U256;
use conflux_provider::{
    BalanceCheckRequest, BlockHashOrEpochNumber, CoreAddress, EpochNumber,
    EstimateGasAndCollateralRequest,
};
use serde::Deserialize;

use super::{ConfluxRpcError, ConfluxSimulationProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoreSpaceResourceEstimate {
    pub gas_limit: AlloyU256,
    pub storage_limit: AlloyU256,
}

impl ConfluxSimulationProvider {
    pub(crate) async fn eth_get_transaction_count(
        &self,
        address: AlloyAddress,
        block: BlockId,
    ) -> Result<u64, ConfluxRpcError> {
        self.espace_provider_at(block)
            .get_transaction_count(address)
            .await
            .map_err(|error| ConfluxRpcError::Espace {
                operation: "eth_getTransactionCount",
                source: error,
            })
    }

    pub(crate) async fn eth_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_provider
            .raw_request("eth_gasPrice".into(), NoParams::default())
            .await
            .map_err(|error| ConfluxRpcError::Espace {
                operation: "eth_gasPrice",
                source: error,
            })
    }

    pub(crate) async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_provider
            .raw_request("eth_maxPriorityFeePerGas".into(), NoParams::default())
            .await
            .map_err(|error| ConfluxRpcError::Espace {
                operation: "eth_maxPriorityFeePerGas",
                source: error,
            })
    }

    pub(crate) async fn eth_estimate_gas(
        &self,
        transaction: &simulation_core::transaction::TransactionRequest,
        block: BlockId,
    ) -> Result<U256, ConfluxRpcError> {
        let estimate = self
            .espace_provider
            .raw_request("eth_estimateGas".into(), (transaction, block))
            .await
            .map_err(|error| ConfluxRpcError::Espace {
                operation: "eth_estimateGas",
                source: error,
            })?;
        Ok(estimate)
    }

    pub(crate) async fn cfx_get_next_nonce(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<U256, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_getNextNonce",
            self.core_space_provider
                .cfx_get_next_nonce(address, selector),
        )
        .await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub(crate) async fn cfx_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        let value =
            Self::core_request("cfx_gasPrice", self.core_space_provider.cfx_gas_price()).await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub(crate) async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_maxPriorityFeePerGas",
            self.core_space_provider.cfx_max_priority_fee_per_gas(),
        )
        .await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub(crate) async fn cfx_estimate_gas_and_collateral(
        &self,
        request: EstimateGasAndCollateralRequest,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceResourceEstimate, ConfluxRpcError> {
        let estimate = Self::core_request(
            "cfx_estimateGasAndCollateral",
            self.core_space_provider
                .cfx_estimate_gas_and_collateral(request, epoch),
        )
        .await?;

        Ok(CoreSpaceResourceEstimate {
            gas_limit: estimate.gas_limit,
            storage_limit: estimate.storage_collateralized,
        })
    }

    pub(crate) async fn cfx_check_balance_against_transaction(
        &self,
        account: CoreAddress,
        contract: CoreAddress,
        gas_limit: AlloyU256,
        gas_price: AlloyU256,
        storage_limit: u64,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceBalanceCheck, ConfluxRpcError> {
        let request = BalanceCheckRequest {
            account,
            contract,
            gas_limit,
            gas_price,
            storage_limit: alloy_u256_from_u64(storage_limit),
        };
        let result = Self::core_request(
            "cfx_checkBalanceAgainstTransaction",
            self.core_space_provider
                .cfx_check_balance_against_transaction(request, epoch),
        )
        .await?;
        Ok(CoreSpaceBalanceCheck {
            will_pay_collateral: result.will_pay_collateral,
        })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceBalanceCheck {
    pub(crate) will_pay_collateral: bool,
}
