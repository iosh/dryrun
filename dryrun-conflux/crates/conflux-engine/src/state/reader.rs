use std::{collections::HashMap, sync::Arc};

use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_storage::{Error as StorageError, Result as StorageResult};
use tokio::sync::Mutex;

use crate::state::{
    ConfluxStatePoint,
    native_internal::{NativeInternalStateItem, SponsorWhitelistStorageKey, decode_abi_bool},
    provider::{RemoteStateProvider, RemoteStateProviderError},
    rpc_types::{EspaceAccountSnapshot, NativeGlobalSnapshot},
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
    native_globals: NativeGlobalSnapshot,
    espace_account_cache: Mutex<HashMap<Address, Arc<EspaceAccountSnapshot>>>,
}

impl RemoteStateReader {
    pub(crate) async fn prepare(
        state_point: ConfluxStatePoint,
        provider: Arc<dyn RemoteStateProvider>,
    ) -> StorageResult<Self> {
        let native_epoch = state_point.native_epoch();
        let native_globals = provider
            .get_native_global_snapshot(native_epoch.clone())
            .await
            .map_err(|error| {
                Self::provider_error_at(&state_point, "get_native_global_snapshot", error)
            })?;

        Ok(Self {
            state_point,
            native_epoch,
            provider,
            native_globals,
            espace_account_cache: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) fn state_point(&self) -> &ConfluxStatePoint {
        &self.state_point
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
            NativeStateItem::InterestRate => {
                Ok(Some(encode_native_u256(self.native_globals.interest_rate)))
            }
            NativeStateItem::AccumulateInterestRate => Ok(Some(encode_native_u256(
                self.native_globals.accumulate_interest_rate,
            ))),
            NativeStateItem::TotalIssued => Ok(Some(encode_native_u256(
                self.native_globals.supply.total_issued,
            ))),
            NativeStateItem::TotalStaking => Ok(Some(encode_native_u256(
                self.native_globals.supply.total_staking,
            ))),
            NativeStateItem::TotalEvmToken => Ok(Some(encode_native_u256(
                self.native_globals.supply.total_espace_tokens,
            ))),
            NativeStateItem::TotalStorage => Ok(Some(encode_native_u256(
                self.native_globals.supply.total_collateral,
            ))),
            NativeStateItem::UsedStoragePoints => Ok(Some(encode_native_u256(
                self.native_globals.collateral.used_storage_points
                    * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
            ))),
            NativeStateItem::ConvertedStoragePoints => Ok(Some(encode_native_u256(
                self.native_globals.collateral.converted_storage_points
                    * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
            ))),
            NativeStateItem::TotalPosStaking => Ok(Some(encode_native_u256(
                self.native_globals.pos_economics.total_pos_staking_tokens,
            ))),
            NativeStateItem::DistributablePosInterest => Ok(Some(encode_native_u256(
                self.native_globals.pos_economics.distributable_pos_interest,
            ))),
            NativeStateItem::LastDistributeBlock => Ok(Some(encode_native_u256(U256::from(
                self.native_globals
                    .pos_economics
                    .last_distribute_block
                    .as_u64(),
            )))),
            NativeStateItem::PowBaseReward => Ok(Some(encode_native_u256(
                self.native_globals.vote_params.pow_base_reward,
            ))),
            NativeStateItem::TotalBurnt1559 => {
                Ok(Some(encode_native_u256(self.native_globals.fee_burnt)))
            }
            NativeStateItem::BaseFeeProp => Ok(Some(encode_native_u256(
                self.native_globals.vote_params.base_fee_share_prop,
            ))),
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

    async fn espace_account_snapshot(
        &self,
        address: Address,
    ) -> StorageResult<Arc<EspaceAccountSnapshot>> {
        if let Some(snapshot) = self
            .espace_account_cache
            .lock()
            .await
            .get(&address)
            .cloned()
        {
            return Ok(snapshot);
        }

        let snapshot = Arc::new(
            self.provider
                .get_espace_account_snapshot(self.espace_block(), address)
                .await
                .map_err(|error| self.provider_error("get_espace_account_snapshot", error))?,
        );
        let mut cache = self.espace_account_cache.lock().await;

        Ok(Arc::clone(cache.entry(address).or_insert_with(|| snapshot)))
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

    async fn fetch_espace_account(&self, address: Address) -> StorageResult<StateRead> {
        let snapshot = self.espace_account_snapshot(address).await?;

        Ok(encode_espace_account(
            snapshot.balance,
            snapshot.nonce,
            snapshot.code.as_ref(),
        ))
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
        let snapshot = self.espace_account_snapshot(address).await?;

        if snapshot.code.is_empty() {
            return Ok(None);
        }

        encode_espace_code(expected_code_hash, Arc::clone(&snapshot.code))
            .map(Some)
            .map_err(|error| self.encoding_error("encode_espace_code", error))
    }

    fn provider_error(
        &self,
        operation: &'static str,
        error: RemoteStateProviderError,
    ) -> StorageError {
        Self::provider_error_at(&self.state_point, operation, error)
    }

    fn provider_error_at(
        state_point: &ConfluxStatePoint,
        operation: &'static str,
        error: RemoteStateProviderError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage provider error: operation={operation}, state={:?},
              reason={error}",
            state_point
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
