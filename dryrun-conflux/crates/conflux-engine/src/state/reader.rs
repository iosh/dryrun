use std::sync::Arc;

use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_storage::{Error as StorageError, Result as StorageResult};

use crate::state::{
    ConfluxStatePoint,
    native_internal::{NativeInternalStateItem, SponsorWhitelistStorageKey, decode_abi_bool},
    provider::{RemoteStateProvider, RemoteStateProviderError},
    state_item::{EspaceStateItem, NativeStateItem, StateItem},
    state_value_encoding::{
        StateValueEncodingError, encode_espace_account, encode_espace_code,
        encode_espace_storage_slot, encode_native_basic_account, encode_native_code,
        encode_native_contract_account, encode_native_storage_slot, encode_native_u256,
        should_encode_native_contract_account,
    },
};
use cfx_types::{Address, H256, U256};

type RawStateValue = Box<[u8]>;
type StateRead = Option<RawStateValue>;

pub(crate) struct RemoteStateReader {
    state_point: ConfluxStatePoint,
    native_epoch: CfxEpochNumber,
    provider: Arc<dyn RemoteStateProvider>,
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
        }
    }

    pub(crate) fn read(&self, item: &StateItem) -> StorageResult<StateRead> {
        match item {
            StateItem::Native(item) => self.read_native(*item),
            StateItem::Espace(item) => self.read_espace(*item),
        }
    }

    fn read_native(&self, item: NativeStateItem) -> StorageResult<StateRead> {
        match item {
            NativeStateItem::Account { address } => self.fetch_native_account(address),
            NativeStateItem::InterestRate => self.fetch_native_interest_rate(),
            NativeStateItem::AccumulateInterestRate => self.fetch_native_accumulate_interest_rate(),
            NativeStateItem::TotalIssued => self.fetch_native_total_issued(),
            NativeStateItem::TotalStaking => self.fetch_native_total_staking(),
            NativeStateItem::TotalEvmToken => self.fetch_native_total_evm_token(),
            NativeStateItem::TotalStorage => self.fetch_native_total_storage(),
            NativeStateItem::UsedStoragePoints => self.fetch_native_used_storage_points(),
            NativeStateItem::ConvertedStoragePoints => self.fetch_native_converted_storage_points(),
            NativeStateItem::TotalPosStaking => self.fetch_native_total_pos_staking(),
            NativeStateItem::DistributablePosInterest => {
                self.fetch_native_distributable_pos_interest()
            }
            NativeStateItem::LastDistributeBlock => self.fetch_native_last_distribute_block(),
            NativeStateItem::PowBaseReward => self.fetch_native_pow_base_reward(),
            NativeStateItem::TotalBurnt1559 => self.fetch_native_total_burnt_1559(),
            NativeStateItem::BaseFeeProp => self.fetch_native_base_fee_prop(),
            NativeStateItem::InternalContractStorage(item) => {
                self.fetch_native_internal_storage(item)
            }
            NativeStateItem::StorageSlot { address, slot } => {
                self.fetch_native_storage_slot(address, slot)
            }
            NativeStateItem::Code { address, code_hash } => {
                self.fetch_native_code(address, code_hash)
            }
        }
    }

    fn read_espace(&self, item: EspaceStateItem) -> StorageResult<StateRead> {
        match item {
            EspaceStateItem::Account { address } => self.fetch_espace_account(address),
            EspaceStateItem::StorageSlot { address, slot } => {
                self.fetch_espace_storage_slot(address, slot)
            }
            EspaceStateItem::Code { address, code_hash } => {
                self.fetch_espace_code(address, code_hash)
            }
        }
    }

    fn native_epoch(&self) -> CfxEpochNumber {
        self.native_epoch.clone()
    }

    fn espace_block(&self) -> EthBlockId {
        self.state_point.espace_block()
    }

    fn fetch_native_account(&self, address: Address) -> StorageResult<StateRead> {
        let account = self
            .provider
            .get_native_account(self.native_epoch(), address)
            .map_err(|error| self.provider_error("get_native_account", error))?;

        if should_encode_native_contract_account(address, account.code_hash) {
            let sponsor_info = self
                .provider
                .get_native_sponsor_info(self.native_epoch(), address)
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

    fn fetch_native_storage_slot(&self, address: Address, slot: H256) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_storage_at(self.native_epoch(), address, slot)
            .map_err(|error| self.provider_error("get_native_storage_at", error))?;

        Ok(value.map(encode_native_storage_slot))
    }

    fn fetch_native_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let code = self
            .provider
            .get_native_code_at(self.native_epoch(), address)
            .map_err(|error| self.provider_error("get_native_code_at", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        encode_native_code(expected_code_hash, address, code)
            .map(Some)
            .map_err(|error| self.encoding_error("encode_native_code", error))
    }

    fn fetch_native_internal_storage(
        &self,
        item: NativeInternalStateItem,
    ) -> StorageResult<StateRead> {
        match item {
            NativeInternalStateItem::SponsorWhitelist(key) => {
                self.fetch_native_sponsor_whitelist_storage(key)
            }
        }
    }

    fn fetch_native_sponsor_whitelist_storage(
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
            .and_then(|value| decode_abi_bool(value, "cfx_call"))
            .map_err(|error| self.provider_error("call_native_sponsor_whitelist", error))?;

        Ok(is_user_whitelisted.then_some(encode_native_storage_slot(U256::one())))
    }

    fn fetch_native_interest_rate(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_interest_rate(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_accumulate_interest_rate(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_accumulate_interest_rate(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_accumulate_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_total_issued(&self) -> StorageResult<StateRead> {
        let supply_info = self
            .provider
            .get_native_supply_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_issued)))
    }

    fn fetch_native_total_staking(&self) -> StorageResult<StateRead> {
        let supply_info = self
            .provider
            .get_native_supply_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_staking)))
    }

    fn fetch_native_total_evm_token(&self) -> StorageResult<StateRead> {
        let supply_info = self
            .provider
            .get_native_supply_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_espace_tokens)))
    }

    fn fetch_native_total_storage(&self) -> StorageResult<StateRead> {
        let supply_info = self
            .provider
            .get_native_supply_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_collateral)))
    }

    fn fetch_native_used_storage_points(&self) -> StorageResult<StateRead> {
        let collateral_info = self
            .provider
            .get_native_collateral_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_collateral_info", error))?;

        Ok(Some(encode_native_u256(
            collateral_info.used_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    fn fetch_native_converted_storage_points(&self) -> StorageResult<StateRead> {
        let collateral_info = self
            .provider
            .get_native_collateral_info(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_collateral_info", error))?;

        Ok(Some(encode_native_u256(
            collateral_info.converted_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    fn fetch_native_total_pos_staking(&self) -> StorageResult<StateRead> {
        let pos_economics = self
            .provider
            .get_native_pos_economics(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(
            pos_economics.total_pos_staking_tokens,
        )))
    }

    fn fetch_native_distributable_pos_interest(&self) -> StorageResult<StateRead> {
        let pos_economics = self
            .provider
            .get_native_pos_economics(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(
            pos_economics.distributable_pos_interest,
        )))
    }

    fn fetch_native_last_distribute_block(&self) -> StorageResult<StateRead> {
        let pos_economics = self
            .provider
            .get_native_pos_economics(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(U256::from(
            pos_economics.last_distribute_block.as_u64(),
        ))))
    }

    fn fetch_native_pow_base_reward(&self) -> StorageResult<StateRead> {
        let vote_params = self
            .provider
            .get_native_vote_params(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_vote_params", error))?;

        Ok(Some(encode_native_u256(vote_params.pow_base_reward)))
    }

    fn fetch_native_total_burnt_1559(&self) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_native_fee_burnt(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_fee_burnt", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_base_fee_prop(&self) -> StorageResult<StateRead> {
        let vote_params = self
            .provider
            .get_native_vote_params(self.native_epoch())
            .map_err(|error| self.provider_error("get_native_vote_params", error))?;

        Ok(Some(encode_native_u256(vote_params.base_fee_share_prop)))
    }

    fn fetch_espace_account(&self, address: Address) -> StorageResult<StateRead> {
        let block = self.espace_block();

        let balance = self
            .provider
            .get_espace_balance(block, address)
            .map_err(|error| self.provider_error("get_espace_balance", error))?;

        let nonce = self
            .provider
            .get_espace_transaction_count(block, address)
            .map_err(|error| self.provider_error("get_espace_transaction_count", error))?;

        let code = self
            .provider
            .get_espace_code_at(block, address)
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        Ok(encode_espace_account(balance, nonce, code))
    }

    fn fetch_espace_storage_slot(&self, address: Address, slot: H256) -> StorageResult<StateRead> {
        let value = self
            .provider
            .get_espace_storage_at(self.espace_block(), address, slot)
            .map_err(|error| self.provider_error("get_espace_storage_at", error))?;

        Ok(value.map(encode_espace_storage_slot))
    }

    fn fetch_espace_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<StateRead> {
        let code = self
            .provider
            .get_espace_code_at(self.espace_block(), address)
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
