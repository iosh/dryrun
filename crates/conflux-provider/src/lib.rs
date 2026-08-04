mod address;
mod batch;
mod cfx;
mod error;
mod filter;
mod pos;
mod types;

use alloy_primitives::{B256, U256};
use alloy_rpc_client::RpcClient;
use serde::Deserialize;

pub use address::{AddressError, CoreAddress, Network, NetworkError};
pub use batch::{BatchCall, CoreBatch};
pub use error::ConfluxProviderError;
pub use types::{
    BalanceCheck, BalanceCheckRequest, BlockHashOrEpochNumber, CoreAccessListItem, CoreAccount,
    CoreBlockTransactions, CoreCollateralInfo, CoreFeeHistory, CoreFilterChange,
    CoreFilterChangeReorg, CoreFilterChanges, CoreFilterId, CoreFilterLog, CoreLog, CoreLogFilter,
    CoreMptValue, CorePendingInfo, CorePendingTransactions, CorePoSEconomics, CoreReceipt,
    CoreRewardInfo, CoreRpcBlock, CoreRpcTransaction, CoreSpace, CoreSponsorInfo, CoreStatus,
    CoreStorageChange, CoreStorageRoot, CoreSupplyInfo, CoreTransactionRequest,
    CoreTransactionType, CoreVariadic, CoreVoteParams, DepositInfo, EpochNumber,
    EstimateGasAndCollateralRequest, GasAndCollateralEstimate, PosAccount, PosBlockFull,
    PosBlockInfo, PosBlockNumber, PosCommittee, PosCommitteeState, PosConflictingVotes,
    PosDecision, PosDisputePayload, PosElectionPayload, PosEpochReward, PosEpochState,
    PosLedgerInfo, PosLedgerInfoWithSignatures, PosNodeLockStatus, PosNodeVotingPower,
    PosRegisterPayload, PosRetirePayload, PosReward, PosSignature, PosStatus, PosTermData,
    PosTransaction, PosTransactionPayload, PosTransactionStatus, PosTransactionType,
    PosUpdateVotingPowerPayload, PosValidatorConsensusInfo, PosValidatorVerifier,
    PosVotePowerState, SelectorError, VoteStakeInfo,
};

const CORE_BATCH_NAME: &str = "Core Space typed batch";

#[derive(Debug, Clone)]
pub struct ConfluxProvider {
    client: RpcClient,
}

impl ConfluxProvider {
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    pub(crate) fn client(&self) -> &RpcClient {
        &self.client
    }

    pub fn batch(&self) -> CoreBatch<'_> {
        CoreBatch::new(self)
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

    pub(crate) fn decode_account(
        &self,
        method: &'static str,
        wire: CoreAccountWire,
    ) -> Result<CoreAccount, ConfluxProviderError> {
        Ok(CoreAccount {
            address: self.decode_address(method, &wire.address)?,
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
        CoreAddress::parse(value).map_err(|source| ConfluxProviderError::Address { method, source })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreAccountWire {
    address: String,
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
