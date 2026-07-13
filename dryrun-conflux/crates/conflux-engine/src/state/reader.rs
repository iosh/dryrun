use std::sync::Arc;

use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_storage::{Error as StorageError, Result as StorageResult};
use tokio::sync::OnceCell;

use crate::state::{
    ConfluxStatePoint,
    native_internal::{NativeInternalStateItem, SponsorWhitelistStorageKey, decode_abi_bool},
    provider::{RemoteStateProvider, RemoteStateProviderError},
    rpc_types::{
        NativePoSEconomics, NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
    },
    state_item::{EspaceStateItem, NativeStateItem, StateItem},
    state_value_encoding::{
        StateValueEncodingError, encode_espace_account, encode_espace_code,
        encode_espace_storage_slot, encode_native_basic_account, encode_native_code,
        encode_native_contract_account, encode_native_deposit_list, encode_native_storage_slot,
        encode_native_u256, encode_native_vote_list, should_encode_native_contract_account,
    },
};
use cfx_types::{Address, H256, U256};

type RawStateValue = Box<[u8]>;
type StateRead = Option<RawStateValue>;

pub(crate) struct RemoteStateReader {
    state_point: ConfluxStatePoint,
    native_epoch: CfxEpochNumber,
    provider: Arc<dyn RemoteStateProvider>,
    native_supply_info_cache: OnceCell<NativeSupplyInfo>,
    native_storage_collateral_info_cache: OnceCell<NativeStorageCollateralInfo>,
    native_pos_economics_cache: OnceCell<NativePoSEconomics>,
    native_vote_params_info_cache: OnceCell<NativeVoteParamsInfo>,
}

impl RemoteStateReader {
    pub(crate) fn new(
        state_point: ConfluxStatePoint,
        provider: Arc<dyn RemoteStateProvider>,
    ) -> Self {
        let native_epoch = state_point.native_epoch();
        Self {
            state_point,
            native_epoch,
            provider,
            native_supply_info_cache: OnceCell::new(),
            native_storage_collateral_info_cache: OnceCell::new(),
            native_pos_economics_cache: OnceCell::new(),
            native_vote_params_info_cache: OnceCell::new(),
        }
    }

    pub(crate) async fn read(&self, item: &StateItem) -> StorageResult<StateRead> {
        match item {
            StateItem::Native(item) => self.read_native(*item).await,
            StateItem::Espace(item) => self.read_espace(*item).await,
        }
    }

    async fn read_native(&self, item: NativeStateItem) -> StorageResult<StateRead> {
        match item {
            NativeStateItem::Account { address } => self.fetch_native_account(address).await,
            NativeStateItem::DepositList { address } => {
                self.fetch_native_deposit_list(address).await
            }
            NativeStateItem::VoteList { address } => self.fetch_native_vote_list(address).await,
            NativeStateItem::InterestRate => self.fetch_native_interest_rate().await,
            NativeStateItem::AccumulateInterestRate => {
                self.fetch_native_accumulate_interest_rate().await
            }
            NativeStateItem::TotalIssued => self.fetch_native_total_issued().await,
            NativeStateItem::TotalStaking => self.fetch_native_total_staking().await,
            NativeStateItem::TotalEvmToken => self.fetch_native_total_evm_token().await,
            NativeStateItem::TotalStorage => self.fetch_native_total_storage().await,
            NativeStateItem::UsedStoragePoints => self.fetch_native_used_storage_points().await,
            NativeStateItem::ConvertedStoragePoints => {
                self.fetch_native_converted_storage_points().await
            }
            NativeStateItem::TotalPosStaking => self.fetch_native_total_pos_staking().await,
            NativeStateItem::DistributablePosInterest => {
                self.fetch_native_distributable_pos_interest().await
            }
            NativeStateItem::LastDistributeBlock => self.fetch_native_last_distribute_block().await,
            NativeStateItem::PowBaseReward => self.fetch_native_pow_base_reward().await,
            NativeStateItem::TotalBurnt1559 => self.fetch_native_total_burnt_1559().await,
            NativeStateItem::BaseFeeProp => self.fetch_native_base_fee_prop().await,
            NativeStateItem::InternalContractStorage(item) => {
                self.fetch_native_internal_storage(item).await
            }
            NativeStateItem::StorageSlot { address, slot } => {
                self.fetch_native_storage_slot(address, slot).await
            }
            NativeStateItem::Code { address, code_hash } => {
                self.fetch_native_code(address, code_hash).await
            }
        }
    }

    async fn read_espace(&self, item: EspaceStateItem) -> StorageResult<StateRead> {
        match item {
            EspaceStateItem::Account { address } => self.fetch_espace_account(address).await,
            EspaceStateItem::StorageSlot { address, slot } => {
                self.fetch_espace_storage_slot(address, slot).await
            }
            EspaceStateItem::Code { address, code_hash } => {
                self.fetch_espace_code(address, code_hash).await
            }
        }
    }

    fn native_epoch(&self) -> CfxEpochNumber {
        self.native_epoch.clone()
    }

    fn espace_block(&self) -> EthBlockId {
        self.state_point.espace_block()
    }

    async fn fetch_native_account(&self, address: Address) -> StorageResult<StateRead> {
        let account = self
            .provider
            .get_native_account(self.native_epoch(), address)
            .await
            .map_err(|error| self.provider_error("get_native_account", error))?;

        if should_encode_native_contract_account(address, account.code_hash) {
            let sponsor_info = self
                .provider
                .get_native_sponsor_info(self.native_epoch(), address)
                .await
                .map_err(|error| self.provider_error("get_native_sponsor_info", error))?;

            return Ok(encode_native_contract_account(
                account.balance,
                account.nonce,
                account.code_hash,
                account.staking_balance,
                account.collateral_for_storage,
                account.accumulated_interest_return,
                account.admin.hex_address,
                sponsor_info,
            ));
        }

        Ok(encode_native_basic_account(
            account.balance,
            account.nonce,
            account.staking_balance,
            account.collateral_for_storage,
            account.accumulated_interest_return,
        ))
    }

    async fn fetch_native_deposit_list(&self, address: Address) -> StorageResult<StateRead> {
        let deposits = self
            .provider
            .get_native_deposit_list(self.native_epoch(), address)
            .await
            .map_err(|error| self.provider_error("get_native_deposit_list", error))?;

        Ok(encode_native_deposit_list(deposits))
    }

    async fn fetch_native_vote_list(&self, address: Address) -> StorageResult<StateRead> {
        let votes = self
            .provider
            .get_native_vote_list(self.native_epoch(), address)
            .await
            .map_err(|error| self.provider_error("get_native_vote_list", error))?;

        Ok(encode_native_vote_list(votes))
    }

    async fn fetch_native_storage_slot(
        &self,
        address: Address,
        slot: H256,
    ) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_storage_at(self.native_epoch(), address, slot)
            .await
            .map_err(|error| self.provider_error("get_native_storage_at", error))?;

        Ok(value.map(encode_native_storage_slot))
    }

    async fn fetch_native_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let code = self
            .provider
            .get_native_code_at(self.native_epoch(), address)
            .await
            .map_err(|error| self.provider_error("get_native_code_at", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        encode_native_code(expected_code_hash, address, code)
            .map(Some)
            .map_err(|error| self.encoding_error("encode_native_code", error))
    }

    async fn fetch_native_internal_storage(
        &self,
        item: NativeInternalStateItem,
    ) -> StorageResult<StateRead> {
        match item {
            NativeInternalStateItem::SponsorWhitelist(key) => {
                self.fetch_native_sponsor_whitelist_storage(key).await
            }
        }
    }

    async fn fetch_native_sponsor_whitelist_storage(
        &self,
        key: SponsorWhitelistStorageKey,
    ) -> StorageResult<StateRead> {
        let is_all_whitelisted = self
            .provider
            .call_native(
                self.native_epoch(),
                key.control_contract_address(),
                key.is_all_whitelisted_call_data(),
            )
            .await
            .and_then(|value| decode_abi_bool(value, "cfx_call"))
            .map_err(|error| self.provider_error("call_native_sponsor_whitelist", error))?;

        if key.is_all_whitelist_key() {
            return Ok(is_all_whitelisted.then_some(encode_native_storage_slot(U256::one())));
        }

        // The raw user key is only read after the all-whitelist key is zero.
        if is_all_whitelisted {
            tracing::warn!(
                contract = ?key.contract,
                user = ?key.user,
                "sponsor whitelist user key is approximate because all-whitelist is enabled"
            );
            return Ok(None);
        }

        let is_user_whitelisted = self
            .provider
            .call_native(
                self.native_epoch(),
                key.control_contract_address(),
                key.is_user_whitelisted_call_data(),
            )
            .await
            .and_then(|value| decode_abi_bool(value, "cfx_call"))
            .map_err(|error| self.provider_error("call_native_sponsor_whitelist", error))?;

        Ok(is_user_whitelisted.then_some(encode_native_storage_slot(U256::one())))
    }

    async fn fetch_native_interest_rate(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_interest_rate(self.native_epoch())
            .await
            .map_err(|error| self.provider_error("get_native_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    async fn fetch_native_accumulate_interest_rate(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_accumulate_interest_rate(self.native_epoch())
            .await
            .map_err(|error| self.provider_error("get_native_accumulate_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    async fn fetch_native_total_issued(&self) -> StorageResult<StateRead> {
        let supply_info = self.native_supply_info().await?;

        Ok(Some(encode_native_u256(supply_info.total_issued)))
    }

    async fn fetch_native_total_staking(&self) -> StorageResult<StateRead> {
        let supply_info = self.native_supply_info().await?;

        Ok(Some(encode_native_u256(supply_info.total_staking)))
    }

    async fn fetch_native_total_evm_token(&self) -> StorageResult<StateRead> {
        let supply_info = self.native_supply_info().await?;

        Ok(Some(encode_native_u256(supply_info.total_espace_tokens)))
    }

    async fn fetch_native_total_storage(&self) -> StorageResult<StateRead> {
        let supply_info = self.native_supply_info().await?;

        Ok(Some(encode_native_u256(supply_info.total_collateral)))
    }

    async fn fetch_native_used_storage_points(&self) -> StorageResult<StateRead> {
        let collateral_info = self.native_storage_collateral_info().await?;

        Ok(Some(encode_native_u256(
            collateral_info.used_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    async fn fetch_native_converted_storage_points(&self) -> StorageResult<StateRead> {
        let collateral_info = self.native_storage_collateral_info().await?;

        Ok(Some(encode_native_u256(
            collateral_info.converted_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    async fn fetch_native_total_pos_staking(&self) -> StorageResult<StateRead> {
        let pos_economics = self.native_pos_economics().await?;

        Ok(Some(encode_native_u256(
            pos_economics.total_pos_staking_tokens,
        )))
    }

    async fn fetch_native_distributable_pos_interest(&self) -> StorageResult<StateRead> {
        let pos_economics = self.native_pos_economics().await?;

        Ok(Some(encode_native_u256(
            pos_economics.distributable_pos_interest,
        )))
    }

    async fn fetch_native_last_distribute_block(&self) -> StorageResult<StateRead> {
        let pos_economics = self.native_pos_economics().await?;

        Ok(Some(encode_native_u256(U256::from(
            pos_economics.last_distribute_block.as_u64(),
        ))))
    }

    async fn fetch_native_pow_base_reward(&self) -> StorageResult<StateRead> {
        let vote_params = self.native_vote_params_info().await?;

        Ok(Some(encode_native_u256(vote_params.pow_base_reward)))
    }

    async fn fetch_native_total_burnt_1559(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_fee_burnt(self.native_epoch())
            .await
            .map_err(|error| self.provider_error("get_native_fee_burnt", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    async fn fetch_native_base_fee_prop(&self) -> StorageResult<StateRead> {
        let vote_params = self.native_vote_params_info().await?;

        Ok(Some(encode_native_u256(vote_params.base_fee_share_prop)))
    }

    async fn native_supply_info(&self) -> StorageResult<&NativeSupplyInfo> {
        self.native_supply_info_cache
            .get_or_try_init(|| async {
                self.provider
                    .get_native_supply_info(self.native_epoch())
                    .await
                    .map_err(|error| self.provider_error("get_native_supply_info", error))
            })
            .await
    }

    async fn native_storage_collateral_info(&self) -> StorageResult<&NativeStorageCollateralInfo> {
        self.native_storage_collateral_info_cache
            .get_or_try_init(|| async {
                self.provider
                    .get_native_collateral_info(self.native_epoch())
                    .await
                    .map_err(|error| self.provider_error("get_native_collateral_info", error))
            })
            .await
    }

    async fn native_pos_economics(&self) -> StorageResult<&NativePoSEconomics> {
        self.native_pos_economics_cache
            .get_or_try_init(|| async {
                self.provider
                    .get_native_pos_economics(self.native_epoch())
                    .await
                    .map_err(|error| self.provider_error("get_native_pos_economics", error))
            })
            .await
    }

    async fn native_vote_params_info(&self) -> StorageResult<&NativeVoteParamsInfo> {
        self.native_vote_params_info_cache
            .get_or_try_init(|| async {
                self.provider
                    .get_native_vote_params(self.native_epoch())
                    .await
                    .map_err(|error| self.provider_error("get_native_vote_params", error))
            })
            .await
    }

    async fn fetch_espace_account(&self, address: Address) -> StorageResult<StateRead> {
        let block = self.espace_block();

        let balance = self
            .provider
            .get_espace_balance(block, address)
            .await
            .map_err(|error| self.provider_error("get_espace_balance", error))?;

        let nonce = self
            .provider
            .get_espace_transaction_count(block, address)
            .await
            .map_err(|error| self.provider_error("get_espace_transaction_count", error))?;

        let code = self
            .provider
            .get_espace_code_at(block, address)
            .await
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        Ok(encode_espace_account(balance, nonce, code))
    }

    async fn fetch_espace_storage_slot(
        &self,
        address: Address,
        slot: H256,
    ) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_espace_storage_at(self.espace_block(), address, slot)
            .await
            .map_err(|error| self.provider_error("get_espace_storage_at", error))?;

        Ok(value.map(encode_espace_storage_slot))
    }

    async fn fetch_espace_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let code = self
            .provider
            .get_espace_code_at(self.espace_block(), address)
            .await
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        encode_espace_code(expected_code_hash, code)
            .map(Some)
            .map_err(|error| self.encoding_error("encode_espace_code", error))
    }

    fn provider_error(
        &self,
        operation: &'static str,
        error: RemoteStateProviderError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage provider error: operation={operation}, state={:?},
              reason={error}",
            self.state_point
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn encoding_error(
        &self,
        operation: &'static str,
        error: StateValueEncodingError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage value encoding error: operation={operation}, state={:?}, reason={error}",
            self.state_point
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }
}
