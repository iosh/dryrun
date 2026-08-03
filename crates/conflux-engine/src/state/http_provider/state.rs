use std::sync::Arc;

use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::BlockId;
use cfx_types::{Address, H256, U256};
use conflux_provider::{BlockHashOrEpochNumber, CoreCallRequest};
use jsonrpsee::{core::params::BatchRequestBuilder, rpc_params};
use primitives::{DepositInfo, VoteStakeInfo};

use crate::state::{
    ConfluxRpcError,
    rpc_types::{
        CoreSpaceAccountState, CoreSpaceGlobals, CoreSpacePoSEconomics, CoreSpaceRpcAccount,
        CoreSpaceSponsorInfo, CoreSpaceStorageCollateralInfo, CoreSpaceSupplyInfo,
        CoreSpaceVoteParamsInfo, EspaceAccountData,
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
        let epoch = Self::provider_epoch(epoch)?;
        let mut batch = self.core_space_provider.batch();
        let interest_rate = batch
            .cfx_get_interest_rate(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getInterestRate", error))?;
        let accumulate_interest_rate =
            batch
                .cfx_get_accumulate_interest_rate(epoch)
                .map_err(|error| {
                    Self::convert_provider_error("cfx_getAccumulateInterestRate", error)
                })?;
        let supply = batch
            .cfx_get_supply_info(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getSupplyInfo", error))?;
        let collateral = batch
            .cfx_get_collateral_info(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getCollateralInfo", error))?;
        let pos_economics = batch
            .cfx_get_pos_economics(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getPoSEconomics", error))?;
        let vote_params = batch
            .cfx_get_params_from_vote(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getParamsFromVote", error))?;
        let fee_burnt = batch
            .cfx_get_fee_burnt(epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getFeeBurnt", error))?;
        Self::core_request(BATCH_NAME, batch.send()).await?;

        let supply = Self::core_request("cfx_getSupplyInfo", supply).await?;
        let collateral = Self::core_request("cfx_getCollateralInfo", collateral).await?;
        let pos_economics = Self::core_request("cfx_getPoSEconomics", pos_economics).await?;
        let vote_params = Self::core_request("cfx_getParamsFromVote", vote_params).await?;

        Ok(CoreSpaceGlobals {
            interest_rate: crate::primitive::u256_to_cfx(
                Self::core_request("cfx_getInterestRate", interest_rate).await?,
            ),
            accumulate_interest_rate: crate::primitive::u256_to_cfx(
                Self::core_request("cfx_getAccumulateInterestRate", accumulate_interest_rate)
                    .await?,
            ),
            supply: CoreSpaceSupplyInfo {
                total_issued: crate::primitive::u256_to_cfx(supply.total_issued),
                total_staking: crate::primitive::u256_to_cfx(supply.total_staking),
                total_espace_tokens: crate::primitive::u256_to_cfx(supply.total_espace_tokens),
                total_collateral: crate::primitive::u256_to_cfx(supply.total_collateral),
            },
            collateral: CoreSpaceStorageCollateralInfo {
                converted_storage_points: crate::primitive::u256_to_cfx(
                    collateral.converted_storage_points,
                ),
                used_storage_points: crate::primitive::u256_to_cfx(collateral.used_storage_points),
            },
            pos_economics: CoreSpacePoSEconomics {
                total_pos_staking_tokens: crate::primitive::u256_to_cfx(
                    pos_economics.total_pos_staking_tokens,
                ),
                distributable_pos_interest: crate::primitive::u256_to_cfx(
                    pos_economics.distributable_pos_interest,
                ),
                last_distribute_block: cfx_types::U64::from(Self::alloy_u256_to_u64(
                    pos_economics.last_distribute_block,
                    "cfx_getPoSEconomics",
                    "lastDistributeBlock",
                )?),
            },
            vote_params: CoreSpaceVoteParamsInfo {
                pow_base_reward: crate::primitive::u256_to_cfx(vote_params.pow_base_reward),
                base_fee_share_prop: crate::primitive::u256_to_cfx(vote_params.base_fee_share_prop),
            },
            fee_burnt: crate::primitive::u256_to_cfx(
                Self::core_request("cfx_getFeeBurnt", fee_burnt).await?,
            ),
        })
    }

    pub(crate) async fn load_core_space_account_state(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceAccountState, ConfluxRpcError> {
        const BATCH_NAME: &str = "Core Space account state";
        let address = self.core_address(address)?;
        let epoch = Self::provider_epoch(epoch)?;
        let mut batch = self.core_space_provider.batch();
        let account = batch
            .cfx_get_account(address, epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getAccount", error))?;
        let collateral = batch
            .cfx_get_collateral_for_storage(address, epoch)
            .map_err(|error| Self::convert_provider_error("cfx_getCollateralForStorage", error))?;
        Self::core_request(BATCH_NAME, batch.send()).await?;
        let account = Self::core_request("cfx_getAccount", account).await?;
        let collateral = Self::core_request("cfx_getCollateralForStorage", collateral).await?;

        Ok(CoreSpaceAccountState {
            account: CoreSpaceRpcAccount {
                balance: crate::primitive::u256_to_cfx(account.balance),
                nonce: crate::primitive::u256_to_cfx(account.nonce),
                code_hash: cfx_types::H256::from_slice(account.code_hash.as_slice()),
                staking_balance: crate::primitive::u256_to_cfx(account.staking_balance),
                total_collateral_for_storage: crate::primitive::u256_to_cfx(
                    account.collateral_for_storage,
                ),
                accumulated_interest_return: crate::primitive::u256_to_cfx(
                    account.accumulated_interest_return,
                ),
                admin: self.provider_address_to_rpc(account.admin)?,
            },
            token_collateral_for_storage: crate::primitive::u256_to_cfx(collateral),
        })
    }

    pub(crate) async fn cfx_get_deposit_list(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<DepositInfo>, ConfluxRpcError> {
        let values = Self::core_request(
            "cfx_getDepositList",
            self.core_space_provider
                .cfx_get_deposit_list(self.core_address(address)?, Self::provider_epoch(epoch)?),
        )
        .await?;
        Ok(values
            .into_iter()
            .map(|value| DepositInfo {
                amount: crate::primitive::u256_to_cfx(value.amount),
                deposit_time: crate::primitive::u256_to_cfx(value.deposit_time),
                accumulated_interest_rate: crate::primitive::u256_to_cfx(
                    value.accumulated_interest_rate,
                ),
            })
            .collect())
    }

    pub(crate) async fn cfx_get_vote_list(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<VoteStakeInfo>, ConfluxRpcError> {
        let values = Self::core_request(
            "cfx_getVoteList",
            self.core_space_provider
                .cfx_get_vote_list(self.core_address(address)?, Self::provider_epoch(epoch)?),
        )
        .await?;
        Ok(values
            .into_iter()
            .map(|value| VoteStakeInfo {
                amount: crate::primitive::u256_to_cfx(value.amount),
                unlock_block_number: crate::primitive::u256_to_cfx(value.unlock_block_number),
            })
            .collect())
    }

    pub(crate) async fn cfx_get_sponsor_info(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceSponsorInfo, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_getSponsorInfo",
            self.core_space_provider
                .cfx_get_sponsor_info(self.core_address(address)?, Self::provider_epoch(epoch)?),
        )
        .await?;
        Ok(CoreSpaceSponsorInfo {
            sponsor_for_gas: self.provider_address_to_rpc(value.sponsor_for_gas)?,
            sponsor_for_collateral: self.provider_address_to_rpc(value.sponsor_for_collateral)?,
            sponsor_gas_bound: crate::primitive::u256_to_cfx(value.sponsor_gas_bound),
            sponsor_balance_for_gas: crate::primitive::u256_to_cfx(value.sponsor_balance_for_gas),
            sponsor_balance_for_collateral: crate::primitive::u256_to_cfx(
                value.sponsor_balance_for_collateral,
            ),
            available_storage_point_units: crate::primitive::u256_to_cfx(
                value.available_storage_points,
            ),
        })
    }

    pub(crate) async fn cfx_get_code(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<Vec<u8>, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_getCode",
            self.core_space_provider.cfx_get_code(
                self.core_address(address)?,
                BlockHashOrEpochNumber::Epoch(Self::provider_epoch(epoch)?),
            ),
        )
        .await?;
        Ok(value.to_vec())
    }

    pub(crate) async fn cfx_get_storage_at(
        &self,
        address: Address,
        slot: H256,
        epoch: EpochNumber,
    ) -> Result<Option<U256>, ConfluxRpcError> {
        let value = Self::core_request(
            "cfx_getStorageAt",
            self.core_space_provider.cfx_get_storage_at(
                self.core_address(address)?,
                crate::primitive::b256_from_cfx(slot),
                BlockHashOrEpochNumber::Epoch(Self::provider_epoch(epoch)?),
            ),
        )
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
        let value = Self::core_request(
            "cfx_call",
            self.core_space_provider.cfx_call(
                CoreCallRequest {
                    to: self.core_address(to)?,
                    data: data.into(),
                },
                BlockHashOrEpochNumber::Epoch(Self::provider_epoch(epoch)?),
            ),
        )
        .await?;
        Ok(value.to_vec())
    }
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
