use async_trait::async_trait;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::{Address, H256, U64, U256};
use jsonrpsee::rpc_params;
use primitives::AccessListItem;
use serde::{Deserialize, Serialize};

use crate::{
    ConfluxTransactionBody, ConfluxTransactionVariant,
    state::provider::{
        ConfluxTransactionProvider, CoreSpaceResourceEstimate, RemoteStateProviderError,
    },
};

use super::HttpConfluxProvider;

#[async_trait]
impl ConfluxTransactionProvider for HttpConfluxProvider {
    async fn eth_get_transaction_count(
        &self,
        address: Address,
        block: BlockId,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getTransactionCount", rpc_params![address, block])
            .await
    }

    async fn eth_gas_price(&self) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request("eth_gasPrice", rpc_params![]).await
    }

    async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request("eth_maxPriorityFeePerGas", rpc_params![])
            .await
    }

    async fn eth_estimate_gas(
        &self,
        block: BlockId,
        transaction: &ConfluxTransactionBody,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request(
            "eth_estimateGas",
            rpc_params![eth_estimate_gas_request(transaction), block],
        )
        .await
    }

    async fn cfx_get_next_nonce(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        self.core_space_rpc_request("cfx_getNextNonce", rpc_params![address, epoch])
            .await
    }

    async fn cfx_gas_price(&self) -> Result<U256, RemoteStateProviderError> {
        self.core_space_rpc_request("cfx_gasPrice", rpc_params![])
            .await
    }

    async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, RemoteStateProviderError> {
        self.core_space_rpc_request("cfx_maxPriorityFeePerGas", rpc_params![])
            .await
    }

    async fn cfx_estimate_gas_and_collateral(
        &self,
        epoch: EpochNumber,
        transaction: &ConfluxTransactionBody,
        epoch_height: u64,
        gas_limit: Option<U256>,
        storage_limit: Option<u64>,
    ) -> Result<CoreSpaceResourceEstimate, RemoteStateProviderError> {
        let request = self.cfx_estimate_gas_and_collateral_request(
            transaction,
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

fn eth_estimate_gas_request(
    transaction: &ConfluxTransactionBody,
) -> EstimateRpcRequest<Address, AccessListItem> {
    let mut request = EstimateRpcRequest {
        from: transaction.from,
        to: transaction.to,
        gas_price: None,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        gas: None,
        value: transaction.value,
        data: RpcBytes::new(transaction.data.clone()),
        nonce: transaction.nonce,
        storage_limit: None,
        access_list: None,
        transaction_type: 0.into(),
        chain_id: transaction.chain_id.into(),
        epoch_height: None,
    };

    match &transaction.variant {
        ConfluxTransactionVariant::Legacy { gas_price } => {
            request.gas_price = Some(*gas_price);
        }
        ConfluxTransactionVariant::AccessList {
            gas_price,
            access_list,
        } => {
            request.gas_price = Some(*gas_price);
            request.access_list = Some(access_list.clone());
            request.transaction_type = 1.into();
        }
        ConfluxTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.access_list = Some(access_list.clone());
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
        transaction: &ConfluxTransactionBody,
        epoch_height: u64,
        gas_limit: Option<U256>,
        storage_limit: Option<u64>,
    ) -> Result<
        EstimateRpcRequest<RpcAddress, CoreEstimateRpcAccessListItem>,
        RemoteStateProviderError,
    > {
        let cfx_access_list = |items: &[AccessListItem]| {
            items
                .iter()
                .map(|item| {
                    Ok(CoreEstimateRpcAccessListItem {
                        address: self.cfx_rpc_address(item.address)?,
                        storage_keys: item.storage_keys.clone(),
                    })
                })
                .collect::<Result<Vec<_>, RemoteStateProviderError>>()
        };
        let mut request = EstimateRpcRequest {
            from: self.cfx_rpc_address(transaction.from)?,
            to: transaction
                .to
                .map(|address| self.cfx_rpc_address(address))
                .transpose()?,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            gas: gas_limit,
            value: transaction.value,
            data: RpcBytes::new(transaction.data.clone()),
            nonce: transaction.nonce,
            storage_limit: storage_limit.map(U64::from),
            access_list: None,
            transaction_type: 0.into(),
            chain_id: transaction.chain_id.into(),
            epoch_height: Some(epoch_height.into()),
        };

        match &transaction.variant {
            ConfluxTransactionVariant::Legacy { gas_price } => {
                request.gas_price = Some(*gas_price);
            }
            ConfluxTransactionVariant::AccessList {
                gas_price,
                access_list,
            } => {
                request.gas_price = Some(*gas_price);
                request.access_list = Some(cfx_access_list(access_list)?);
                request.transaction_type = 1.into();
            }
            ConfluxTransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => {
                request.max_fee_per_gas = Some(*max_fee_per_gas);
                request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
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
