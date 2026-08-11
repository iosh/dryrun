use crate::{
    core_space::{CoreSpaceAccessListItem, CoreSpaceCompleteTransactionVariant},
    espace::EspaceCompleteTransactionVariant,
    primitive::{access_list_to_cfx, address_to_cfx, alloy_u256_from_u64, u256_to_cfx},
};
use alloy::{
    eips::BlockId,
    primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256},
    providers::Provider,
    rpc::{client::NoParams, types::TransactionInput},
};
use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::TransactionRequest as EspaceRpcTransactionRequest;
use cfx_types::{U64, U256};
use conflux_provider::{
    BalanceCheckRequest, BlockHashOrEpochNumber, CoreAccessListItem, CoreAddress,
    CoreTransactionType, EstimateGasAndCollateralRequest,
};
use primitives::transaction::AuthorizationListItem;
use serde::Deserialize;

use super::{ConfluxRpcError, ConfluxSimulationProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CoreSpaceResourceEstimate {
    pub gas_limit: AlloyU256,
    pub storage_limit: AlloyU256,
}

pub(crate) struct EspaceEstimateTransaction<'a> {
    pub(crate) from: AlloyAddress,
    pub(crate) to: Option<AlloyAddress>,
    pub(crate) nonce: u64,
    pub(crate) value: AlloyU256,
    pub(crate) data: &'a AlloyBytes,
    pub(crate) chain_id: u64,
    pub(crate) variant: &'a EspaceCompleteTransactionVariant,
}

pub(crate) struct CoreSpaceEstimateTransaction<'a> {
    pub(crate) from: CoreAddress,
    pub(crate) to: Option<CoreAddress>,
    pub(crate) nonce: AlloyU256,
    pub(crate) value: AlloyU256,
    pub(crate) data: &'a AlloyBytes,
    pub(crate) chain_id: u32,
    pub(crate) variant: &'a CoreSpaceCompleteTransactionVariant,
    pub(crate) epoch_height: u64,
    pub(crate) gas_limit: Option<AlloyU256>,
    pub(crate) storage_limit: Option<u64>,
}

impl ConfluxSimulationProvider {
    pub(crate) async fn eth_get_transaction_count(
        &self,
        address: AlloyAddress,
        block: BlockId,
    ) -> Result<U256, ConfluxRpcError> {
        let nonce = self
            .espace_provider_at(block)
            .get_transaction_count(address)
            .await
            .map_err(|error| ConfluxRpcError {
                operation: "eth_getTransactionCount",
                reason: error.to_string(),
            })?;
        Ok(U256::from(nonce))
    }

    pub(crate) async fn eth_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_provider
            .raw_request("eth_gasPrice".into(), NoParams::default())
            .await
            .map_err(|error| ConfluxRpcError {
                operation: "eth_gasPrice",
                reason: error.to_string(),
            })
    }

    pub(crate) async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_provider
            .raw_request("eth_maxPriorityFeePerGas".into(), NoParams::default())
            .await
            .map_err(|error| ConfluxRpcError {
                operation: "eth_maxPriorityFeePerGas",
                reason: error.to_string(),
            })
    }

    pub(crate) async fn eth_estimate_gas(
        &self,
        transaction: EspaceEstimateTransaction<'_>,
        block: BlockId,
    ) -> Result<U256, ConfluxRpcError> {
        let estimate = self
            .espace_provider
            .raw_request(
                "eth_estimateGas".into(),
                (espace_estimate_gas_request(transaction), block),
            )
            .await
            .map_err(|error| ConfluxRpcError {
                operation: "eth_estimateGas",
                reason: error.to_string(),
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
        transaction: CoreSpaceEstimateTransaction<'_>,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceResourceEstimate, ConfluxRpcError> {
        let request = core_space_estimate_request(transaction);
        let estimate = Self::core_request(
            "cfx_estimateGasAndCollateral",
            self.core_space_provider
                .cfx_estimate_gas_and_collateral(request, Self::provider_epoch(epoch)?),
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
                .cfx_check_balance_against_transaction(request, Self::provider_epoch(epoch)?),
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

fn espace_estimate_gas_request(
    transaction: EspaceEstimateTransaction<'_>,
) -> EspaceRpcTransactionRequest {
    let mut request = EspaceRpcTransactionRequest {
        from: Some(address_to_cfx(transaction.from)),
        to: transaction.to.map(address_to_cfx),
        value: Some(u256_to_cfx(transaction.value)),
        input: TransactionInput::new(transaction.data.clone()),
        nonce: Some(U256::from(transaction.nonce)),
        chain_id: Some(U256::from(transaction.chain_id)),
        ..Default::default()
    };

    match transaction.variant {
        EspaceCompleteTransactionVariant::Legacy { gas_price } => {
            request.transaction_type = Some(U64::from(0));
            request.gas_price = Some(u256_to_cfx(*gas_price));
        }
        EspaceCompleteTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => {
            request.transaction_type = Some(U64::from(1));
            request.gas_price = Some(u256_to_cfx(*gas_price));
            request.access_list = Some(access_list_to_cfx(access_list.to_vec()));
        }
        EspaceCompleteTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.transaction_type = Some(U64::from(2));
            request.max_fee_per_gas = Some(u256_to_cfx(*max_fee_per_gas));
            request.max_priority_fee_per_gas = Some(u256_to_cfx(*max_priority_fee_per_gas));
            request.access_list = Some(access_list_to_cfx(access_list.to_vec()));
        }
        EspaceCompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
            authorization_list,
        } => {
            request.transaction_type = Some(U64::from(4));
            request.max_fee_per_gas = Some(u256_to_cfx(*max_fee_per_gas));
            request.max_priority_fee_per_gas = Some(u256_to_cfx(*max_priority_fee_per_gas));
            request.access_list = Some(access_list_to_cfx(access_list.to_vec()));
            request.authorization_list = Some(
                authorization_list
                    .iter()
                    .map(|authorization| {
                        let inner = authorization.inner();
                        AuthorizationListItem {
                            chain_id: u256_to_cfx(inner.chain_id),
                            address: address_to_cfx(inner.address),
                            nonce: inner.nonce,
                            y_parity: authorization.y_parity(),
                            r: u256_to_cfx(authorization.r()),
                            s: u256_to_cfx(authorization.s()),
                        }
                        .into()
                    })
                    .collect(),
            );
        }
    }

    request
}

fn core_space_estimate_request(
    transaction: CoreSpaceEstimateTransaction<'_>,
) -> EstimateGasAndCollateralRequest {
    let core_access_list = |items: &[CoreSpaceAccessListItem]| {
        items
            .iter()
            .map(|item| CoreAccessListItem {
                address: item.address,
                storage_keys: item.storage_keys.clone(),
            })
            .collect::<Vec<_>>()
    };
    let mut request = EstimateGasAndCollateralRequest {
        from: transaction.from,
        to: transaction.to,
        gas_price: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        gas: transaction.gas_limit,
        value: transaction.value,
        data: transaction.data.clone(),
        nonce: transaction.nonce,
        storage_limit: transaction.storage_limit.map(alloy_u256_from_u64),
        access_list: None,
        transaction_type: CoreTransactionType::Legacy,
        chain_id: AlloyU256::from(transaction.chain_id),
        epoch_height: Some(alloy_u256_from_u64(transaction.epoch_height)),
    };

    match transaction.variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { gas_price } => {
            request.gas_price = Some(*gas_price);
        }
        CoreSpaceCompleteTransactionVariant::Cip2930 {
            gas_price,
            access_list,
        } => {
            request.gas_price = Some(*gas_price);
            request.access_list = Some(core_access_list(access_list));
            request.transaction_type = CoreTransactionType::AccessList;
        }
        CoreSpaceCompleteTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.access_list = Some(core_access_list(access_list));
            request.transaction_type = CoreTransactionType::DynamicFee;
        }
    }

    request
}
