mod address;
mod batch;
mod error;
mod types;

use alloy_primitives::{B256, Bytes, U256};
use alloy_rpc_client::RpcClient;
use serde::Deserialize;

pub use address::{AddressError, CoreAddress, Network, NetworkError};
pub use batch::{BatchCall, CoreBatch};
pub use error::ConfluxProviderError;
pub use types::{
    BalanceCheck, BalanceCheckRequest, BlockHashOrEpochNumber, CoreAccessListItem, CoreAccount,
    CoreBlock, CoreCallRequest, CoreCollateralInfo, CorePoSEconomics, CoreSponsorInfo,
    CoreSupplyInfo, CoreTransactionType, CoreVoteParams, DepositInfo, EpochNumber,
    EstimateGasAndCollateralRequest, GasAndCollateralEstimate, PosBlock, PosPivotDecision,
    SelectorError, VoteStakeInfo,
};

const CORE_BATCH_NAME: &str = "Core Space typed batch";

#[derive(Debug, Clone)]
pub struct ConfluxProvider {
    client: RpcClient,
    network: Network,
}

impl ConfluxProvider {
    pub fn new(client: RpcClient, network: Network) -> Result<Self, ConfluxProviderError> {
        network
            .validate()
            .map_err(|source| ConfluxProviderError::Address {
                method: "provider construction",
                source: AddressError::Network(source),
            })?;
        Ok(Self { client, network })
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub(crate) fn client(&self) -> &RpcClient {
        &self.client
    }

    pub fn batch(&self) -> CoreBatch<'_> {
        CoreBatch::new(self)
    }

    pub async fn cfx_get_block_by_epoch_number(
        &self,
        epoch: EpochNumber,
    ) -> Result<Option<CoreBlock>, ConfluxProviderError> {
        let wire: Option<CoreBlockWire> = self
            .request("cfx_getBlockByEpochNumber", (epoch, false))
            .await?;
        wire.map(|wire| self.decode_block("cfx_getBlockByEpochNumber", wire))
            .transpose()
    }

    pub async fn pos_get_block_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<PosBlock>, ConfluxProviderError> {
        self.request("pos_getBlockByHash", (hash,)).await
    }

    pub async fn cfx_get_next_nonce(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        let address = self.check_address("cfx_getNextNonce", address)?;
        self.request("cfx_getNextNonce", (address, selector)).await
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, ConfluxProviderError> {
        self.request_noparams("cfx_gasPrice").await
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxProviderError> {
        self.request_noparams("cfx_maxPriorityFeePerGas").await
    }

    pub async fn cfx_estimate_gas_and_collateral(
        &self,
        request: EstimateGasAndCollateralRequest,
        epoch: EpochNumber,
    ) -> Result<GasAndCollateralEstimate, ConfluxProviderError> {
        self.check_address("cfx_estimateGasAndCollateral", request.from)?;
        if let Some(to) = request.to {
            self.check_address("cfx_estimateGasAndCollateral", to)?;
        }
        if let Some(access_list) = &request.access_list {
            for item in access_list {
                self.check_address("cfx_estimateGasAndCollateral", item.address)?;
            }
        }
        self.request("cfx_estimateGasAndCollateral", (request, epoch))
            .await
    }

    pub async fn cfx_check_balance_against_transaction(
        &self,
        request: BalanceCheckRequest,
        epoch: EpochNumber,
    ) -> Result<BalanceCheck, ConfluxProviderError> {
        let account = self.check_address("cfx_checkBalanceAgainstTransaction", request.account)?;
        let contract =
            self.check_address("cfx_checkBalanceAgainstTransaction", request.contract)?;
        self.request(
            "cfx_checkBalanceAgainstTransaction",
            (
                account,
                contract,
                request.gas_limit,
                request.gas_price,
                request.storage_limit,
                Some(epoch),
            ),
        )
        .await
    }

    pub async fn cfx_get_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getInterestRate", (epoch,)).await
    }

    pub async fn cfx_get_accumulate_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getAccumulateInterestRate", (epoch,))
            .await
    }

    pub async fn cfx_get_supply_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSupplyInfo, ConfluxProviderError> {
        self.request("cfx_getSupplyInfo", (epoch,)).await
    }

    pub async fn cfx_get_collateral_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreCollateralInfo, ConfluxProviderError> {
        self.request("cfx_getCollateralInfo", (epoch,)).await
    }

    pub async fn cfx_get_pos_economics(
        &self,
        epoch: EpochNumber,
    ) -> Result<CorePoSEconomics, ConfluxProviderError> {
        self.request("cfx_getPoSEconomics", (epoch,)).await
    }

    pub async fn cfx_get_params_from_vote(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreVoteParams, ConfluxProviderError> {
        self.request("cfx_getParamsFromVote", (epoch,)).await
    }

    pub async fn cfx_get_fee_burnt(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getFeeBurnt", (epoch,)).await
    }

    pub async fn cfx_get_account(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<CoreAccount, ConfluxProviderError> {
        let address = self.check_address("cfx_getAccount", address)?;
        let wire: CoreAccountWire = self.request("cfx_getAccount", (address, epoch)).await?;
        self.decode_account("cfx_getAccount", address, wire)
    }

    pub async fn cfx_get_collateral_for_storage(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        let address = self.check_address("cfx_getCollateralForStorage", address)?;
        self.request("cfx_getCollateralForStorage", (address, epoch))
            .await
    }

    pub async fn cfx_get_deposit_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<DepositInfo>, ConfluxProviderError> {
        let address = self.check_address("cfx_getDepositList", address)?;
        self.request("cfx_getDepositList", (address, epoch)).await
    }

    pub async fn cfx_get_vote_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<VoteStakeInfo>, ConfluxProviderError> {
        let address = self.check_address("cfx_getVoteList", address)?;
        self.request("cfx_getVoteList", (address, epoch)).await
    }

    pub async fn cfx_get_sponsor_info(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<CoreSponsorInfo, ConfluxProviderError> {
        let address = self.check_address("cfx_getSponsorInfo", address)?;
        let wire: CoreSponsorInfoWire =
            self.request("cfx_getSponsorInfo", (address, epoch)).await?;
        self.decode_sponsor_info("cfx_getSponsorInfo", wire)
    }

    pub async fn cfx_get_code(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<Bytes, ConfluxProviderError> {
        let address = self.check_address("cfx_getCode", address)?;
        self.request("cfx_getCode", (address, selector)).await
    }

    pub async fn cfx_get_storage_at(
        &self,
        address: CoreAddress,
        slot: B256,
        selector: BlockHashOrEpochNumber,
    ) -> Result<Option<Bytes>, ConfluxProviderError> {
        let address = self.check_address("cfx_getStorageAt", address)?;
        self.request(
            "cfx_getStorageAt",
            (address, U256::from_be_bytes(slot.0), selector),
        )
        .await
    }

    pub async fn cfx_call(
        &self,
        request: CoreCallRequest,
        selector: BlockHashOrEpochNumber,
    ) -> Result<Bytes, ConfluxProviderError> {
        self.check_address("cfx_call", request.to)?;
        self.request("cfx_call", (request, selector)).await
    }

    async fn request<Params, Response>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<Response, ConfluxProviderError>
    where
        Params: alloy_json_rpc::RpcSend,
        Response: alloy_json_rpc::RpcRecv,
    {
        self.client
            .request(method, params)
            .await
            .map_err(|error| error::classify_alloy_rpc_error(method, error))
    }

    async fn request_noparams<Response>(
        &self,
        method: &'static str,
    ) -> Result<Response, ConfluxProviderError>
    where
        Response: alloy_json_rpc::RpcRecv,
    {
        self.client
            .request_noparams::<Response>(method)
            .await
            .map_err(|error| error::classify_alloy_rpc_error(method, error))
    }

    pub(crate) fn check_address(
        &self,
        method: &'static str,
        address: CoreAddress,
    ) -> Result<CoreAddress, ConfluxProviderError> {
        if address.network() != self.network {
            return Err(ConfluxProviderError::NetworkMismatch {
                method,
                expected: self.network,
                actual: address.network(),
            });
        }
        Ok(address)
    }

    fn decode_block(
        &self,
        method: &'static str,
        wire: CoreBlockWire,
    ) -> Result<CoreBlock, ConfluxProviderError> {
        Ok(CoreBlock {
            hash: wire.hash,
            height: wire.height,
            miner: self.decode_address(method, &wire.miner)?,
            block_number: wire.block_number,
            base_fee_per_gas: wire.base_fee_per_gas,
            timestamp: wire.timestamp,
            pos_reference: wire.pos_reference,
        })
    }

    pub(crate) fn decode_account(
        &self,
        method: &'static str,
        requested: CoreAddress,
        wire: CoreAccountWire,
    ) -> Result<CoreAccount, ConfluxProviderError> {
        Ok(CoreAccount {
            address: wire
                .address
                .as_deref()
                .map(|address| self.decode_address(method, address))
                .transpose()?
                .unwrap_or(requested),
            balance: wire.balance,
            nonce: wire.nonce,
            code_hash: wire.code_hash,
            staking_balance: wire.staking_balance,
            collateral_for_storage: wire.collateral_for_storage,
            accumulated_interest_return: wire.accumulated_interest_return,
            admin: self.decode_address(method, &wire.admin)?,
        })
    }

    fn decode_sponsor_info(
        &self,
        method: &'static str,
        wire: CoreSponsorInfoWire,
    ) -> Result<CoreSponsorInfo, ConfluxProviderError> {
        Ok(CoreSponsorInfo {
            sponsor_for_gas: self.decode_address(method, &wire.sponsor_for_gas)?,
            sponsor_for_collateral: self.decode_address(method, &wire.sponsor_for_collateral)?,
            sponsor_gas_bound: wire.sponsor_gas_bound,
            sponsor_balance_for_gas: wire.sponsor_balance_for_gas,
            sponsor_balance_for_collateral: wire.sponsor_balance_for_collateral,
            available_storage_points: wire.available_storage_points,
            used_storage_points: wire.used_storage_points,
        })
    }

    pub(crate) fn decode_address(
        &self,
        method: &'static str,
        value: &str,
    ) -> Result<CoreAddress, ConfluxProviderError> {
        let address = CoreAddress::parse(value)
            .map_err(|source| ConfluxProviderError::Address { method, source })?;
        self.check_address(method, address)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreBlockWire {
    hash: B256,
    height: U256,
    miner: String,
    block_number: Option<U256>,
    base_fee_per_gas: Option<U256>,
    timestamp: U256,
    pos_reference: Option<B256>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreAccountWire {
    address: Option<String>,
    balance: U256,
    nonce: U256,
    code_hash: B256,
    staking_balance: U256,
    collateral_for_storage: U256,
    accumulated_interest_return: U256,
    admin: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSponsorInfoWire {
    sponsor_for_gas: String,
    sponsor_for_collateral: String,
    sponsor_gas_bound: U256,
    sponsor_balance_for_gas: U256,
    sponsor_balance_for_collateral: U256,
    available_storage_points: U256,
    used_storage_points: U256,
}
