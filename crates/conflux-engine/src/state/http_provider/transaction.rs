use alloy_primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256};
use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::BlockId;
use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::{Address, U64, U256};
use conflux_provider::{
    BalanceCheckRequest, CoreAccessListItem, CoreTransactionType, EstimateGasAndCollateralRequest,
};
use jsonrpsee::rpc_params;
use serde::{Deserialize, Serialize};
use simulation_transaction::{AccessListItem, TransactionVariant};

use crate::primitive::{
    access_list_ref_to_cfx, address_to_cfx, alloy_u256_from_u64, alloy_u256_from_u128, u256_to_cfx,
};

use super::{ConfluxRpcError, HttpConfluxProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpaceResourceEstimate {
    pub gas_limit: U256,
    pub storage_limit: u64,
}

impl HttpConfluxProvider {
    pub async fn eth_get_transaction_count(
        &self,
        address: AlloyAddress,
        block: BlockId,
    ) -> Result<U256, ConfluxRpcError> {
        self.espace_rpc_request(
            "eth_getTransactionCount",
            rpc_params![address_to_cfx(address), block],
        )
        .await
    }

    pub async fn eth_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_rpc_request("eth_gasPrice", rpc_params![]).await
    }

    pub async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        self.espace_rpc_request("eth_maxPriorityFeePerGas", rpc_params![])
            .await
    }

    pub async fn eth_estimate_gas(
        &self,
        from: AlloyAddress,
        to: Option<AlloyAddress>,
        nonce: u64,
        value: AlloyU256,
        data: &AlloyBytes,
        chain_id: u64,
        variant: &TransactionVariant,
        block: BlockId,
    ) -> Result<U256, ConfluxRpcError> {
        self.espace_rpc_request(
            "eth_estimateGas",
            rpc_params![
                eth_estimate_gas_request(EstimateTransaction {
                    from,
                    to,
                    nonce,
                    value,
                    data,
                    chain_id,
                    variant,
                }),
                block
            ],
        )
        .await
    }

    pub async fn cfx_get_next_nonce(
        &self,
        address: AlloyAddress,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxRpcError> {
        let address = self.core_address(address_to_cfx(address))?;
        let selector =
            conflux_provider::BlockHashOrEpochNumber::Epoch(Self::provider_epoch(epoch)?);
        let value = Self::core_request(
            "cfx_getNextNonce",
            self.core_space_provider
                .cfx_get_next_nonce(address, selector),
        )
        .await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        let value =
            Self::core_request("cfx_gasPrice", self.core_space_provider.cfx_gas_price()).await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_maxPriorityFeePerGas",
            self.core_space_provider.cfx_max_priority_fee_per_gas(),
        )
        .await?;
        Ok(crate::primitive::u256_to_cfx(value))
    }

    pub async fn cfx_estimate_gas_and_collateral(
        &self,
        from: AlloyAddress,
        to: Option<AlloyAddress>,
        nonce: u64,
        value: AlloyU256,
        data: &AlloyBytes,
        chain_id: u64,
        variant: &TransactionVariant,
        epoch_height: u64,
        gas_limit: Option<u64>,
        storage_limit: Option<u64>,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceResourceEstimate, ConfluxRpcError> {
        let request = self.cfx_estimate_gas_and_collateral_request(
            EstimateTransaction {
                from,
                to,
                nonce,
                value,
                data,
                chain_id,
                variant,
            },
            epoch_height,
            gas_limit,
            storage_limit,
        )?;
        let estimate = Self::core_request(
            "cfx_estimateGasAndCollateral",
            self.core_space_provider
                .cfx_estimate_gas_and_collateral(request, Self::provider_epoch(epoch)?),
        )
        .await?;

        Ok(CoreSpaceResourceEstimate {
            gas_limit: crate::primitive::u256_to_cfx(estimate.gas_limit),
            storage_limit: Self::alloy_u256_to_u64(
                estimate.storage_collateralized,
                "cfx_estimateGasAndCollateral",
                "storageCollateralized",
            )?,
        })
    }

    pub(crate) async fn cfx_check_balance_against_transaction(
        &self,
        account: Address,
        contract: Address,
        gas_limit: u64,
        gas_price: u128,
        storage_limit: u64,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceBalanceCheck, ConfluxRpcError> {
        let request = BalanceCheckRequest {
            account: self.core_address(account)?,
            contract: self.core_address(contract)?,
            gas_limit: alloy_u256_from_u64(gas_limit),
            gas_price: alloy_u256_from_u128(gas_price),
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

#[derive(Clone, Copy)]
struct EstimateTransaction<'a> {
    from: AlloyAddress,
    to: Option<AlloyAddress>,
    nonce: u64,
    value: AlloyU256,
    data: &'a AlloyBytes,
    chain_id: u64,
    variant: &'a TransactionVariant,
}

fn eth_estimate_gas_request(
    transaction: EstimateTransaction<'_>,
) -> EstimateRpcRequest<Address, primitives::AccessListItem> {
    let mut request = EstimateRpcRequest {
        from: address_to_cfx(transaction.from),
        to: transaction.to.map(address_to_cfx),
        gas_price: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        gas: None,
        value: u256_to_cfx(transaction.value),
        data: RpcBytes::new(transaction.data.to_vec()),
        nonce: transaction.nonce.into(),
        storage_limit: None,
        access_list: None,
        transaction_type: 0.into(),
        chain_id: transaction.chain_id.into(),
        epoch_height: None,
    };

    match transaction.variant {
        TransactionVariant::Legacy { gas_price } => {
            request.gas_price = Some((*gas_price).into());
        }
        TransactionVariant::AccessList {
            gas_price,
            access_list,
        } => {
            request.gas_price = Some((*gas_price).into());
            request.access_list = Some(access_list_ref_to_cfx(access_list));
            request.transaction_type = 1.into();
        }
        TransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.max_fee_per_gas = Some((*max_fee_per_gas).into());
            request.max_priority_fee_per_gas = Some((*max_priority_fee_per_gas).into());
            request.access_list = Some(access_list_ref_to_cfx(access_list));
            request.transaction_type = 2.into();
        }
    }

    request
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EstimateRpcRequest<A, I> {
    from: A,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<A>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas_price: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_priority_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas: Option<U256>,
    value: U256,
    data: RpcBytes,
    nonce: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_limit: Option<U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_list: Option<Vec<I>>,
    #[serde(rename = "type")]
    transaction_type: U64,
    chain_id: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch_height: Option<U256>,
}

impl HttpConfluxProvider {
    fn cfx_estimate_gas_and_collateral_request(
        &self,
        transaction: EstimateTransaction<'_>,
        epoch_height: u64,
        gas_limit: Option<u64>,
        storage_limit: Option<u64>,
    ) -> Result<EstimateGasAndCollateralRequest, ConfluxRpcError> {
        let core_access_list = |items: &[AccessListItem]| {
            items
                .iter()
                .map(|item| {
                    Ok(CoreAccessListItem {
                        address: self.core_address(address_to_cfx(item.address))?,
                        storage_keys: item.storage_keys.clone(),
                    })
                })
                .collect::<Result<Vec<_>, ConfluxRpcError>>()
        };
        let mut request = EstimateGasAndCollateralRequest {
            from: self.core_address(address_to_cfx(transaction.from))?,
            to: transaction
                .to
                .map(|address| self.core_address(address_to_cfx(address)))
                .transpose()?,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas: gas_limit.map(alloy_u256_from_u64),
            value: transaction.value,
            data: transaction.data.clone(),
            nonce: alloy_u256_from_u64(transaction.nonce),
            storage_limit: storage_limit.map(alloy_u256_from_u64),
            access_list: None,
            transaction_type: CoreTransactionType::Legacy,
            chain_id: alloy_u256_from_u64(transaction.chain_id),
            epoch_height: Some(alloy_u256_from_u64(epoch_height)),
        };

        match transaction.variant {
            TransactionVariant::Legacy { gas_price } => {
                request.gas_price = Some(alloy_u256_from_u128(*gas_price));
            }
            TransactionVariant::AccessList {
                gas_price,
                access_list,
            } => {
                request.gas_price = Some(alloy_u256_from_u128(*gas_price));
                request.access_list = Some(core_access_list(access_list)?);
                request.transaction_type = CoreTransactionType::AccessList;
            }
            TransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => {
                request.max_fee_per_gas = Some(alloy_u256_from_u128(*max_fee_per_gas));
                request.max_priority_fee_per_gas =
                    Some(alloy_u256_from_u128(*max_priority_fee_per_gas));
                request.access_list = Some(core_access_list(access_list)?);
                request.transaction_type = CoreTransactionType::DynamicFee;
            }
        }

        Ok(request)
    }
}
