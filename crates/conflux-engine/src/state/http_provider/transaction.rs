use alloy_primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256};
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::{Address, H256, U64, U256};
use jsonrpsee::rpc_params;
use serde::{Deserialize, Serialize};
use simulation_transaction::{AccessListItem, TransactionVariant};

use crate::primitive::{access_list_ref_to_cfx, address_to_cfx, b256_to_cfx, u256_to_cfx};

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
        let address = self.cfx_rpc_address(address_to_cfx(address))?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        self.core_space_rpc_request("cfx_getNextNonce", rpc_params![address, epoch])
            .await
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, ConfluxRpcError> {
        self.core_space_rpc_request("cfx_gasPrice", rpc_params![])
            .await
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxRpcError> {
        self.core_space_rpc_request("cfx_maxPriorityFeePerGas", rpc_params![])
            .await
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
        let estimate: CoreEstimateRpcResponse = self
            .core_space_rpc_request("cfx_estimateGasAndCollateral", rpc_params![request, epoch])
            .await?;

        Ok(CoreSpaceResourceEstimate {
            gas_limit: estimate.gas_limit,
            storage_limit: estimate.storage_collateralized.as_u64(),
        })
    }
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
    ) -> Result<EstimateRpcRequest<RpcAddress, CoreEstimateRpcAccessListItem>, ConfluxRpcError>
    {
        let cfx_access_list = |items: &[AccessListItem]| {
            items
                .iter()
                .map(|item| {
                    Ok(CoreEstimateRpcAccessListItem {
                        address: self.cfx_rpc_address(address_to_cfx(item.address))?,
                        storage_keys: item.storage_keys.iter().copied().map(b256_to_cfx).collect(),
                    })
                })
                .collect::<Result<Vec<_>, ConfluxRpcError>>()
        };
        let mut request = EstimateRpcRequest {
            from: self.cfx_rpc_address(address_to_cfx(transaction.from))?,
            to: transaction
                .to
                .map(|address| self.cfx_rpc_address(address_to_cfx(address)))
                .transpose()?,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas: gas_limit.map(U256::from),
            value: u256_to_cfx(transaction.value),
            data: RpcBytes::new(transaction.data.to_vec()),
            nonce: transaction.nonce.into(),
            storage_limit: storage_limit.map(U64::from),
            access_list: None,
            transaction_type: 0.into(),
            chain_id: transaction.chain_id.into(),
            epoch_height: Some(epoch_height.into()),
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
                request.access_list = Some(cfx_access_list(access_list)?);
                request.transaction_type = 1.into();
            }
            TransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => {
                request.max_fee_per_gas = Some((*max_fee_per_gas).into());
                request.max_priority_fee_per_gas = Some((*max_priority_fee_per_gas).into());
                request.access_list = Some(cfx_access_list(access_list)?);
                request.transaction_type = 2.into();
            }
        }

        Ok(request)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreEstimateRpcAccessListItem {
    address: RpcAddress,
    storage_keys: Vec<H256>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoreEstimateRpcResponse {
    gas_limit: U256,
    storage_collateralized: U64,
}
