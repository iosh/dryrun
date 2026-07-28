use std::sync::Arc;

use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::{Address, H256, U256};
use jsonrpsee::{core::params::BatchRequestBuilder, rpc_params};
use primitives::{DepositInfo, VoteStakeInfo};
use serde::Serialize;

use crate::state::{
    ConfluxRpcError,
    rpc_types::{
        CoreSpaceGlobals, CoreSpacePoSEconomics, CoreSpaceRpcAccount, CoreSpaceSponsorInfo,
        CoreSpaceStorageCollateralInfo, CoreSpaceSupplyInfo, CoreSpaceVoteParamsInfo,
        EspaceAccountData,
    },
};

use super::HttpConfluxProvider;

impl HttpConfluxProvider {
    pub(crate) async fn eth_get_storage_at(
        &self,
        address: Address,
        slot: H256,
        block_number: BlockId,
    ) -> Result<Option<U256>, ConfluxRpcError> {
        let value: H256 = self
            .espace_rpc_request(
                "eth_getStorageAt",
                rpc_params![
                    address,
                    U256::from_big_endian(slot.as_bytes()),
                    block_number
                ],
            )
            .await?;

        let value = U256::from_big_endian(value.as_bytes());
        Ok((!value.is_zero()).then_some(value))
    }

    pub(crate) async fn load_espace_account(
        &self,
        address: Address,
        block_number: BlockId,
    ) -> Result<EspaceAccountData, ConfluxRpcError> {
        const BATCH_NAME: &str = "eSpace account";
        const BATCH_LEN: usize = 3;

        let mut batch = BatchRequestBuilder::new();
        Self::insert_batch_request(
            &mut batch,
            "eth_getBalance",
            rpc_params![address, block_number],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "eth_getTransactionCount",
            rpc_params![address, block_number],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "eth_getCode",
            rpc_params![address, block_number],
        )?;

        let response =
            Self::rpc_batch_request(&self.espace_client, "espace", BATCH_NAME, BATCH_LEN, batch)
                .await?;
        Self::validate_batch_len(BATCH_NAME, BATCH_LEN, response.len())?;
        let mut entries = response.into_iter();
        let balance = Self::decode_batch_result(&mut entries, "eth_getBalance")?;
        let nonce = Self::decode_batch_result(&mut entries, "eth_getTransactionCount")?;
        let code: String = Self::decode_batch_result(&mut entries, "eth_getCode")?;

        Ok(EspaceAccountData {
            balance,
            nonce,
            code: Arc::new(decode_rpc_bytes(code, "eth_getCode")?),
        })
    }

    pub(crate) async fn load_core_space_globals(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceGlobals, ConfluxRpcError> {
        const BATCH_NAME: &str = "Core Space globals";
        const BATCH_LEN: usize = 7;

        let mut batch = BatchRequestBuilder::new();
        Self::insert_batch_request(
            &mut batch,
            "cfx_getInterestRate",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getAccumulateInterestRate",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(&mut batch, "cfx_getSupplyInfo", rpc_params![epoch.clone()])?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getCollateralInfo",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getPoSEconomics",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getParamsFromVote",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(&mut batch, "cfx_getFeeBurnt", rpc_params![epoch])?;

        let response = Self::rpc_batch_request(
            &self.core_space_client,
            "core_space",
            BATCH_NAME,
            BATCH_LEN,
            batch,
        )
        .await?;
        Self::validate_batch_len(BATCH_NAME, BATCH_LEN, response.len())?;
        let mut entries = response.into_iter();

        Ok(CoreSpaceGlobals {
            interest_rate: Self::decode_batch_result(&mut entries, "cfx_getInterestRate")?,
            accumulate_interest_rate: Self::decode_batch_result(
                &mut entries,
                "cfx_getAccumulateInterestRate",
            )?,
            supply: Self::decode_batch_result::<CoreSpaceSupplyInfo>(
                &mut entries,
                "cfx_getSupplyInfo",
            )?,
            collateral: Self::decode_batch_result::<CoreSpaceStorageCollateralInfo>(
                &mut entries,
                "cfx_getCollateralInfo",
            )?,
            pos_economics: Self::decode_batch_result::<CoreSpacePoSEconomics>(
                &mut entries,
                "cfx_getPoSEconomics",
            )?,
            vote_params: Self::decode_batch_result::<CoreSpaceVoteParamsInfo>(
                &mut entries,
                "cfx_getParamsFromVote",
            )?,
            fee_burnt: Self::decode_batch_result(&mut entries, "cfx_getFeeBurnt")?,
        })
    }

    pub(crate) async fn cfx_get_account(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceRpcAccount, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getAccount", rpc_params![address, epoch])
            .await
    }

    pub(crate) async fn cfx_get_deposit_list(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<DepositInfo>, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getDepositList", rpc_params![address, epoch])
            .await
    }

    pub(crate) async fn cfx_get_vote_list(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<VoteStakeInfo>, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getVoteList", rpc_params![address, epoch])
            .await
    }

    pub(crate) async fn cfx_get_sponsor_info(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceSponsorInfo, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getSponsorInfo", rpc_params![address, epoch])
            .await
    }

    pub(crate) async fn cfx_get_code(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<u8>, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: String = self
            .core_space_rpc_request("cfx_getCode", rpc_params![address, epoch])
            .await?;

        decode_rpc_bytes(value, "cfx_getCode")
    }

    pub(crate) async fn cfx_get_storage_at(
        &self,
        address: Address,
        slot: H256,
        epoch: EpochNumber,
    ) -> Result<Option<U256>, ConfluxRpcError> {
        let address = self.cfx_rpc_address(address)?;
        let slot = U256::from_big_endian(slot.as_bytes());
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: Option<RpcBytes> = self
            .core_space_rpc_request("cfx_getStorageAt", rpc_params![address, slot, epoch])
            .await?;
        let Some(value) = value else {
            return Ok(None);
        };

        if value.is_empty() {
            return Ok(None);
        }

        if value.len() != 32 {
            return Err(ConfluxRpcError {
                operation: "cfx_getStorageAt",
                reason: format!("expected 32 bytes, got {}", value.len()),
            });
        }

        Ok(Some(U256::from_big_endian(value.as_ref())))
    }

    pub(crate) async fn cfx_call(
        &self,
        to: Address,
        data: Vec<u8>,
        epoch: EpochNumber,
    ) -> Result<Vec<u8>, ConfluxRpcError> {
        let to = self.cfx_rpc_address(to)?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);
        let request = CoreSpaceCallRequest {
            to,
            data: data.into(),
        };

        let value: String = self
            .core_space_rpc_request("cfx_call", rpc_params![request, epoch])
            .await?;
        decode_rpc_bytes(value, "cfx_call")
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceCallRequest {
    to: RpcAddress,
    data: RpcBytes,
}

fn decode_rpc_bytes(value: String, field: &'static str) -> Result<Vec<u8>, ConfluxRpcError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| ConfluxRpcError {
        operation: field,
        reason: "missing 0x prefix".to_owned(),
    })?;

    if digits.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(digits).map_err(|error| ConfluxRpcError {
        operation: field,
        reason: error.to_string(),
    })
}
