use std::sync::Arc;

use async_trait::async_trait;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::{Address, H256, U256};
use jsonrpsee::{core::params::BatchRequestBuilder, rpc_params};
use primitives::{DepositInfo, VoteStakeInfo};
use serde::Serialize;

use crate::state::{
    provider::{ConfluxStateProvider, RemoteStateProviderError},
    rpc_encoding::{RpcStorageWord, decode_rpc_bytes},
    rpc_types::{
        CoreSpaceGlobalSnapshot, CoreSpacePoSEconomics, CoreSpaceRpcAccount, CoreSpaceSponsorInfo,
        CoreSpaceStorageCollateralInfo, CoreSpaceSupplyInfo, CoreSpaceVoteParamsInfo,
        EspaceAccountSnapshot,
    },
};

use super::HttpConfluxProvider;

#[async_trait]
impl ConfluxStateProvider for HttpConfluxProvider {
    async fn eth_get_storage_at(
        &self,
        block_number: BlockId,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
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

    async fn get_espace_account_snapshot(
        &self,
        block_number: BlockId,
        address: Address,
    ) -> Result<EspaceAccountSnapshot, RemoteStateProviderError> {
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

        Ok(EspaceAccountSnapshot {
            balance,
            nonce,
            code: Arc::new(decode_rpc_bytes(code, "eth_getCode")?),
        })
    }

    async fn get_core_space_global_snapshot(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceGlobalSnapshot, RemoteStateProviderError> {
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

        Ok(CoreSpaceGlobalSnapshot {
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

    async fn cfx_get_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceRpcAccount, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getAccount", rpc_params![address, epoch])
            .await
    }

    async fn cfx_get_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getDepositList", rpc_params![address, epoch])
            .await
    }

    async fn cfx_get_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getVoteList", rpc_params![address, epoch])
            .await
    }

    async fn cfx_get_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceSponsorInfo, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;

        self.core_space_rpc_request("cfx_getSponsorInfo", rpc_params![address, epoch])
            .await
    }

    async fn cfx_get_code(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: String = self
            .core_space_rpc_request("cfx_getCode", rpc_params![address, epoch])
            .await?;

        decode_rpc_bytes(value, "cfx_getCode")
    }

    async fn cfx_get_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let address = self.cfx_rpc_address(address)?;
        let slot = U256::from_big_endian(slot.as_bytes());
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: RpcStorageWord = self
            .core_space_rpc_request("cfx_getStorageAt", rpc_params![address, slot, epoch])
            .await?;

        value.into_option_u256()
    }

    async fn cfx_call(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
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
