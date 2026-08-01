use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex as SyncMutex},
};

use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_storage::{Error as StorageError, Result as StorageResult};
use tokio::sync::Mutex as AsyncMutex;

use crate::state::{
    ConfluxRpcError, ConfluxStateAnchor, HttpConfluxProvider,
    core_space_internal::{
        CoreSpaceInternalStateItem, SponsorWhitelistStorageKey, decode_abi_bool,
    },
    rpc_types::{CoreSpaceAccountState, CoreSpaceGlobals, EspaceAccountData},
    state_item::{CoreSpaceStateItem, EspaceStateItem, StateItem},
    state_value_encoding::{
        StateValueEncodingError, encode_code, encode_core_space_basic_account,
        encode_core_space_contract_account, encode_core_space_deposit_list, encode_core_space_u256,
        encode_core_space_vote_list, encode_espace_account, encode_storage_slot,
        should_encode_core_space_contract_account, used_storage_point_collateral,
    },
};
use cfx_types::{Address, H256, U256};
use primitives::VoteStakeInfo;

type RawStateValue = Box<[u8]>;
type StateRead = Option<RawStateValue>;

#[derive(Clone, Default)]
pub(crate) struct StoragePointInitializationUncertainty {
    uncertain_account_addresses: Arc<SyncMutex<HashSet<Address>>>,
}

impl StoragePointInitializationUncertainty {
    fn record_uncertain_account(&self, address: Address) -> StorageResult<()> {
        self.uncertain_account_addresses
            .lock()
            .map_err(|_| Self::lock_error())?
            .insert(address);
        Ok(())
    }

    pub(crate) fn uncertain_account_addresses(&self) -> StorageResult<HashSet<Address>> {
        self.uncertain_account_addresses
            .lock()
            .map_err(|_| Self::lock_error())
            .map(|addresses| addresses.clone())
    }

    fn lock_error() -> StorageError {
        StorageError::Msg(
            "failed to access request-local Core Space storage-point state".to_owned(),
        )
    }
}

/// Vote lists fetched by this request's anchored StateDB reads.
#[derive(Clone, Default)]
pub(crate) struct AnchoredVoteLists {
    vote_lists_by_account: Arc<SyncMutex<HashMap<Address, Vec<VoteStakeInfo>>>>,
}

impl AnchoredVoteLists {
    fn record(&self, address: Address, vote_list: Vec<VoteStakeInfo>) -> StorageResult<()> {
        let mut vote_lists_by_account = self
            .vote_lists_by_account
            .lock()
            .map_err(|_| Self::lock_error())?;
        if let Some(existing) = vote_lists_by_account.get(&address) {
            if existing != &vote_list {
                return Err(StorageError::Msg(format!(
                    "anchored cfx_getVoteList returned inconsistent results for {address:?}"
                )));
            }
            return Ok(());
        }
        vote_lists_by_account.insert(address, vote_list);
        Ok(())
    }

    pub(crate) fn for_account(&self, address: Address) -> StorageResult<Vec<VoteStakeInfo>> {
        self.vote_lists_by_account
            .lock()
            .map_err(|_| Self::lock_error())?
            .get(&address)
            .cloned()
            .ok_or_else(|| {
                StorageError::Msg(format!(
                    "anchored cfx_getVoteList result was not read during execution for {address:?}"
                ))
            })
    }

    fn lock_error() -> StorageError {
        StorageError::Msg("failed to access request-local Core Space vote-list state".to_owned())
    }
}

pub(crate) struct RemoteStateReader {
    state_anchor: ConfluxStateAnchor,
    provider: Arc<HttpConfluxProvider>,
    core_space_globals: CoreSpaceGlobals,
    espace_account_cache: AsyncMutex<HashMap<Address, Arc<EspaceAccountData>>>,
    storage_point_initialization_uncertainty: StoragePointInitializationUncertainty,
    anchored_vote_lists: AnchoredVoteLists,
}

impl RemoteStateReader {
    pub(crate) async fn prepare(
        state_anchor: ConfluxStateAnchor,
        provider: Arc<HttpConfluxProvider>,
    ) -> StorageResult<Self> {
        let core_space_epoch = state_anchor.core_space_epoch();
        let core_space_globals = provider
            .load_core_space_globals(core_space_epoch)
            .await
            .map_err(|error| {
                Self::provider_error_at(&state_anchor, "load_core_space_globals", error)
            })?;

        Ok(Self {
            state_anchor,
            provider,
            core_space_globals,
            espace_account_cache: AsyncMutex::new(HashMap::new()),
            storage_point_initialization_uncertainty:
                StoragePointInitializationUncertainty::default(),
            anchored_vote_lists: AnchoredVoteLists::default(),
        })
    }

    pub(crate) fn state_anchor(&self) -> ConfluxStateAnchor {
        self.state_anchor
    }

    pub(crate) fn storage_point_initialization_uncertainty(
        &self,
    ) -> StoragePointInitializationUncertainty {
        self.storage_point_initialization_uncertainty.clone()
    }

    pub(crate) fn anchored_vote_lists(&self) -> AnchoredVoteLists {
        self.anchored_vote_lists.clone()
    }

    pub(crate) async fn read(&self, item: &StateItem) -> StorageResult<StateRead> {
        match item {
            StateItem::CoreSpace(item) => self.read_core_space(*item).await,
            StateItem::Espace(item) => self.read_espace(*item).await,
        }
    }

    async fn read_core_space(&self, item: CoreSpaceStateItem) -> StorageResult<StateRead> {
        match item {
            CoreSpaceStateItem::Account { address } => self.fetch_core_space_account(address).await,
            CoreSpaceStateItem::DepositList { address } => self
                .provider
                .cfx_get_deposit_list(address, self.core_space_epoch())
                .await
                .map_err(|error| self.provider_error("cfx_getDepositList", error))
                .map(encode_core_space_deposit_list),
            CoreSpaceStateItem::VoteList { address } => {
                let vote_list = self
                    .provider
                    .cfx_get_vote_list(address, self.core_space_epoch())
                    .await
                    .map_err(|error| self.provider_error("cfx_getVoteList", error))?;
                self.anchored_vote_lists
                    .record(address, vote_list.clone())?;
                Ok(encode_core_space_vote_list(vote_list))
            }
            CoreSpaceStateItem::InterestRate => Ok(Some(encode_core_space_u256(
                self.core_space_globals.interest_rate,
            ))),
            CoreSpaceStateItem::AccumulateInterestRate => Ok(Some(encode_core_space_u256(
                self.core_space_globals.accumulate_interest_rate,
            ))),
            CoreSpaceStateItem::TotalIssued => Ok(Some(encode_core_space_u256(
                self.core_space_globals.supply.total_issued,
            ))),
            CoreSpaceStateItem::TotalStaking => Ok(Some(encode_core_space_u256(
                self.core_space_globals.supply.total_staking,
            ))),
            CoreSpaceStateItem::TotalEvmToken => Ok(Some(encode_core_space_u256(
                self.core_space_globals.supply.total_espace_tokens,
            ))),
            CoreSpaceStateItem::TotalStorage => Ok(Some(encode_core_space_u256(
                self.core_space_globals.supply.total_collateral,
            ))),
            CoreSpaceStateItem::UsedStoragePoints => Ok(Some(encode_core_space_u256(
                self.core_space_globals.collateral.used_storage_points
                    * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
            ))),
            CoreSpaceStateItem::ConvertedStoragePoints => Ok(Some(encode_core_space_u256(
                self.core_space_globals.collateral.converted_storage_points
                    * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
            ))),
            CoreSpaceStateItem::TotalPosStaking => Ok(Some(encode_core_space_u256(
                self.core_space_globals
                    .pos_economics
                    .total_pos_staking_tokens,
            ))),
            CoreSpaceStateItem::DistributablePosInterest => Ok(Some(encode_core_space_u256(
                self.core_space_globals
                    .pos_economics
                    .distributable_pos_interest,
            ))),
            CoreSpaceStateItem::LastDistributeBlock => {
                Ok(Some(encode_core_space_u256(U256::from(
                    self.core_space_globals
                        .pos_economics
                        .last_distribute_block
                        .as_u64(),
                ))))
            }
            CoreSpaceStateItem::PowBaseReward => Ok(Some(encode_core_space_u256(
                self.core_space_globals.vote_params.pow_base_reward,
            ))),
            CoreSpaceStateItem::TotalBurnt1559 => Ok(Some(encode_core_space_u256(
                self.core_space_globals.fee_burnt,
            ))),
            CoreSpaceStateItem::BaseFeeProp => Ok(Some(encode_core_space_u256(
                self.core_space_globals.vote_params.base_fee_share_prop,
            ))),
            CoreSpaceStateItem::InternalContractStorage(item) => {
                self.fetch_core_space_internal_storage(item).await
            }
            CoreSpaceStateItem::StorageSlot { address, slot } => {
                // Public Core Space RPC returns the value without its storage owner.
                self.provider
                    .cfx_get_storage_at(address, slot, self.core_space_epoch())
                    .await
                    .map_err(|error| self.provider_error("cfx_getStorageAt", error))
                    .map(|value| value.map(encode_storage_slot))
            }
            CoreSpaceStateItem::Code { address, code_hash } => {
                self.fetch_core_space_code(address, code_hash).await
            }
        }
    }

    async fn read_espace(&self, item: EspaceStateItem) -> StorageResult<StateRead> {
        match item {
            EspaceStateItem::Account { address } => self.fetch_espace_account(address).await,
            EspaceStateItem::StorageSlot { address, slot } => self
                .provider
                .eth_get_storage_at(address, slot, self.espace_block())
                .await
                .map_err(|error| self.provider_error("eth_getStorageAt", error))
                .map(|value| value.map(encode_storage_slot)),
            EspaceStateItem::Code { address, code_hash } => {
                self.fetch_espace_code(address, code_hash).await
            }
        }
    }

    fn core_space_epoch(&self) -> CfxEpochNumber {
        self.state_anchor.core_space_epoch()
    }

    fn espace_block(&self) -> EthBlockId {
        self.state_anchor.espace_block()
    }

    async fn espace_account_data(&self, address: Address) -> StorageResult<Arc<EspaceAccountData>> {
        if let Some(account) = self
            .espace_account_cache
            .lock()
            .await
            .get(&address)
            .cloned()
        {
            return Ok(account);
        }

        let account = Arc::new(
            self.provider
                .load_espace_account(address, self.espace_block())
                .await
                .map_err(|error| self.provider_error("load_espace_account", error))?,
        );
        let mut cache = self.espace_account_cache.lock().await;

        Ok(Arc::clone(cache.entry(address).or_insert_with(|| account)))
    }

    async fn fetch_core_space_account(&self, address: Address) -> StorageResult<StateRead> {
        let CoreSpaceAccountState {
            account,
            token_collateral_for_storage,
        } = self
            .provider
            .load_core_space_account_state(address, self.core_space_epoch())
            .await
            .map_err(|error| self.provider_error("load_core_space_account_state", error))?;
        let used_storage_point_collateral = used_storage_point_collateral(
            account.total_collateral_for_storage,
            token_collateral_for_storage,
        )
        .map_err(|error| self.encoding_error("derive_core_space_collateral", error))?;

        if should_encode_core_space_contract_account(address, account.code_hash) {
            let sponsor_info = self
                .provider
                .cfx_get_sponsor_info(address, self.core_space_epoch())
                .await
                .map_err(|error| self.provider_error("cfx_getSponsorInfo", error))?;

            let storage_point_initialization_is_unknown =
                sponsor_info.available_storage_point_units.is_zero()
                    && used_storage_point_collateral.is_zero();
            let encoded_account = encode_core_space_contract_account(
                account.balance,
                account.nonce,
                account.code_hash,
                account.staking_balance,
                token_collateral_for_storage,
                used_storage_point_collateral,
                account.accumulated_interest_return,
                account.admin.hex_address,
                sponsor_info,
            )
            .map_err(|error| self.encoding_error("encode_core_space_contract_account", error))?;

            if encoded_account.is_some() && storage_point_initialization_is_unknown {
                self.storage_point_initialization_uncertainty
                    .record_uncertain_account(address)?;
            }

            return Ok(encoded_account);
        }

        if !used_storage_point_collateral.is_zero() {
            return Err(self.encoding_error(
                "encode_core_space_basic_account",
                StateValueEncodingError::BasicAccountStoragePointCollateral {
                    value: used_storage_point_collateral,
                },
            ));
        }

        Ok(encode_core_space_basic_account(
            account.balance,
            account.nonce,
            account.staking_balance,
            token_collateral_for_storage,
            account.accumulated_interest_return,
        ))
    }

    async fn fetch_core_space_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let code = self
            .provider
            .cfx_get_code(address, self.core_space_epoch())
            .await
            .map_err(|error| self.provider_error("cfx_getCode", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        // Core Space code keeps the state address as the upstream CodeInfo owner.
        encode_code(expected_code_hash, address, Arc::new(code))
            .map(Some)
            .map_err(|error| self.encoding_error("encode_core_space_code", error))
    }

    async fn fetch_core_space_internal_storage(
        &self,
        item: CoreSpaceInternalStateItem,
    ) -> StorageResult<StateRead> {
        match item {
            CoreSpaceInternalStateItem::SponsorWhitelist(key) => {
                self.fetch_core_space_sponsor_whitelist_storage(key).await
            }
        }
    }

    async fn fetch_core_space_sponsor_whitelist_storage(
        &self,
        key: SponsorWhitelistStorageKey,
    ) -> StorageResult<StateRead> {
        let is_all_whitelisted = self
            .provider
            .cfx_call(
                key.control_contract_address(),
                key.is_all_whitelisted_call_data(),
                self.core_space_epoch(),
            )
            .await
            .and_then(|value| decode_abi_bool(value, "cfx_call"))
            .map_err(|error| self.provider_error("cfx_call", error))?;

        if key.is_all_whitelist_key() {
            return Ok(is_all_whitelisted.then_some(encode_storage_slot(U256::one())));
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
            .cfx_call(
                key.control_contract_address(),
                key.is_user_whitelisted_call_data(),
                self.core_space_epoch(),
            )
            .await
            .and_then(|value| decode_abi_bool(value, "cfx_call"))
            .map_err(|error| self.provider_error("cfx_call", error))?;

        Ok(is_user_whitelisted.then_some(encode_storage_slot(U256::one())))
    }

    async fn fetch_espace_account(&self, address: Address) -> StorageResult<StateRead> {
        let account = self.espace_account_data(address).await?;

        Ok(encode_espace_account(
            account.balance,
            account.nonce,
            account.code.as_ref(),
        ))
    }

    async fn fetch_espace_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let account = self.espace_account_data(address).await?;

        if account.code.is_empty() {
            return Ok(None);
        }

        // Upstream eSpace code values use the zero address as their owner.
        encode_code(
            expected_code_hash,
            Address::zero(),
            Arc::clone(&account.code),
        )
        .map(Some)
        .map_err(|error| self.encoding_error("encode_espace_code", error))
    }

    fn provider_error(&self, operation: &'static str, error: ConfluxRpcError) -> StorageError {
        Self::provider_error_at(&self.state_anchor, operation, error)
    }

    fn provider_error_at(
        state_anchor: &ConfluxStateAnchor,
        operation: &'static str,
        error: ConfluxRpcError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage provider error: operation={operation}, state={:?},
              reason={error}",
            state_anchor
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
            self.state_anchor
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }
}
