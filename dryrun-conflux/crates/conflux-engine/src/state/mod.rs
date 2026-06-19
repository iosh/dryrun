mod codec;
mod provider;
mod request;

pub use self::provider::{
    EspaceRpcBlock, HttpEspaceProvider, NativePoSEconomics, NativeRpcBlock,
    NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo, RemoteStateProvider,
    RemoteStateProviderError,
};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cfx_internal_common::StateRootWithAuxInfo;
use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_statedb::StateDb;
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use cfx_types::{Address, H256, U256};
use primitives::{EpochId, StorageKeyWithSpace};

use crate::state::codec::encode_espace_account;

use self::{
    codec::{
        StateValueCodecError, encode_espace_code, encode_espace_storage_slot,
        encode_native_basic_account, encode_native_u256,
    },
    request::{StateReadRequest, StateReadRequestError},
};

pub fn new_state_db(storage: Box<dyn StorageStateTrait>) -> StateDb {
    StateDb::new(storage)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfluxStateSnapshot {
    pub espace_block_id: String,
    pub native_epoch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    snapshot: ConfluxStateSnapshot,
    key_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
enum CachedRead {
    Present(Box<[u8]>),
    // Only use for states proven missing, not unsupported or failed reads.
    Absent,
}

pub struct RpcBackedStorage {
    snapshot: ConfluxStateSnapshot,
    provider: Arc<dyn RemoteStateProvider>,
    // Simulation writes shadow remote state.
    overlay: Mutex<HashMap<Vec<u8>, CachedRead>>,
    // Per-simulation remote read cache.
    cache: Mutex<HashMap<CacheKey, CachedRead>>,
}

impl RpcBackedStorage {
    pub fn new(snapshot: ConfluxStateSnapshot, provider: Arc<dyn RemoteStateProvider>) -> Self {
        Self {
            snapshot,
            provider,
            overlay: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn unsupported(&self, operation: &'static str, key: StorageKeyWithSpace<'_>) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, snapshot={:?}, key={:?}",
            self.snapshot, key
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_storage_key(
        &self,
        operation: &'static str,
        key: StorageKeyWithSpace<'_>,
        error: StateReadRequestError,
    ) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage key: operation={operation}, snapshot={:?}, key={key:?}, reason={error}",
            self.snapshot
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_operation(&self, operation: &'static str) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, snapshot={:?}",
            self.snapshot
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_commit(&self, epoch: EpochId) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation=commit, snapshot={:?}, epoch={epoch:?}",
            self.snapshot
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn cache_key(&self, access_key: StorageKeyWithSpace<'_>) -> CacheKey {
        CacheKey {
            snapshot: self.snapshot.clone(),
            key_bytes: access_key.to_key_bytes(),
        }
    }

    fn cached_read_to_result(read: CachedRead) -> Option<Box<[u8]>> {
        match read {
            CachedRead::Present(value) => Some(value),
            CachedRead::Absent => None,
        }
    }

    // This is the future handoff point from semantic RPC state into Conflux raw bytes.
    fn fetch_rpc_value(&self, rpc_key: &StateReadRequest) -> StorageResult<Option<Box<[u8]>>> {
        match rpc_key {
            StateReadRequest::NativeInterestRate => self.fetch_native_interest_rate(),
            StateReadRequest::EspaceAccount { address } => self.fetch_espace_account(*address),
            StateReadRequest::EspaceStorageSlot { address, slot } => {
                self.fetch_espace_storage_slot(*address, *slot)
            }
            StateReadRequest::EspaceCode { address, code_hash } => {
                self.fetch_espace_code(*address, *code_hash)
            }
            StateReadRequest::NativeAccumulateInterestRate => {
                self.fetch_native_accumulate_interest_rate()
            }
            StateReadRequest::NativeTotalIssued => self.fetch_native_total_issued(),
            StateReadRequest::NativeTotalStaking => self.fetch_native_total_staking(),
            StateReadRequest::NativeTotalEvmToken => self.fetch_native_total_evm_token(),
            StateReadRequest::NativeTotalStorage => self.fetch_native_total_storage(),
            StateReadRequest::NativeUsedStoragePoints => self.fetch_native_used_storage_points(),
            StateReadRequest::NativeConvertedStoragePoints => {
                self.fetch_native_converted_storage_points()
            }
            StateReadRequest::NativeTotalPosStaking => self.fetch_native_total_pos_staking(),
            StateReadRequest::NativeDistributablePosInterest => {
                self.fetch_native_distributable_pos_interest()
            }
            StateReadRequest::NativeLastDistributeBlock => {
                self.fetch_native_last_distribute_block()
            }
            StateReadRequest::NativePowBaseReward => self.fetch_native_pow_base_reward(),
            StateReadRequest::NativeTotalBurnt1559 => self.fetch_native_total_burnt_1559(),
            StateReadRequest::NativeBaseFeeProp => self.fetch_native_base_fee_prop(),
            StateReadRequest::NativeAccount { address } => self.fetch_native_account(*address),
        }
    }

    fn fetch_native_account(&self, address: Address) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let account = self
            .provider
            .get_native_account(epoch, address)
            .map_err(|error| self.provider_error("get_native_account", error))?;

        Ok(encode_native_basic_account(
            account.balance,
            account.nonce,
            account.staking_balance,
            account.collateral_for_storage,
            account.accumulated_interest_return,
        ))
    }

    fn fetch_native_pow_base_reward(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let vote_params = self
            .provider
            .get_native_vote_params(epoch)
            .map_err(|error| self.provider_error("get_native_vote_params", error))?;

        Ok(Some(encode_native_u256(vote_params.pow_base_reward)))
    }

    fn fetch_native_total_burnt_1559(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let value = self
            .provider
            .get_native_fee_burnt(epoch)
            .map_err(|error| self.provider_error("get_native_fee_burnt", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_base_fee_prop(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let vote_params = self
            .provider
            .get_native_vote_params(epoch)
            .map_err(|error| self.provider_error("get_native_vote_params", error))?;

        Ok(Some(encode_native_u256(vote_params.base_fee_share_prop)))
    }
    fn fetch_native_total_pos_staking(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let pos_economics = self
            .provider
            .get_native_pos_economics(epoch)
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(
            pos_economics.total_pos_staking_tokens,
        )))
    }

    fn fetch_native_distributable_pos_interest(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let pos_economics = self
            .provider
            .get_native_pos_economics(epoch)
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(
            pos_economics.distributable_pos_interest,
        )))
    }

    fn fetch_native_last_distribute_block(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let pos_economics = self
            .provider
            .get_native_pos_economics(epoch)
            .map_err(|error| self.provider_error("get_native_pos_economics", error))?;

        Ok(Some(encode_native_u256(U256::from(
            pos_economics.last_distribute_block.as_u64(),
        ))))
    }

    fn fetch_native_used_storage_points(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let collateral_info = self
            .provider
            .get_native_collateral_info(epoch)
            .map_err(|error| self.provider_error("get_native_collateral_info", error))?;

        Ok(Some(encode_native_u256(
            collateral_info.used_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    fn fetch_native_converted_storage_points(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let collateral_info = self
            .provider
            .get_native_collateral_info(epoch)
            .map_err(|error| self.provider_error("get_native_collateral_info", error))?;

        Ok(Some(encode_native_u256(
            collateral_info.converted_storage_points * *DRIPS_PER_STORAGE_COLLATERAL_UNIT,
        )))
    }

    fn fetch_native_total_storage(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let supply_info = self
            .provider
            .get_native_supply_info(epoch)
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_collateral)))
    }
    fn fetch_native_total_evm_token(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let supply_info = self
            .provider
            .get_native_supply_info(epoch)
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_espace_tokens)))
    }

    fn fetch_espace_account(&self, address: Address) -> StorageResult<Option<Box<[u8]>>> {
        let block_id = self.snapshot.espace_block_id.as_str();

        let balance = self
            .provider
            .get_espace_balance(block_id, address)
            .map_err(|error| self.provider_error("get_espace_balance", error))?;

        let nonce = self
            .provider
            .get_espace_transaction_count(block_id, address)
            .map_err(|error| self.provider_error("get_espace_transaction_count", error))?;

        let code = self
            .provider
            .get_espace_code_at(block_id, address)
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        Ok(encode_espace_account(balance, nonce, code))
    }

    fn fetch_espace_storage_slot(
        &self,
        address: Address,
        slot: H256,
    ) -> StorageResult<Option<Box<[u8]>>> {
        let block_id = self.snapshot.espace_block_id.as_str();

        let value = self
            .provider
            .get_espace_storage_at(block_id, address, slot)
            .map_err(|error| self.provider_error("get_espace_storage_at", error))?;

        Ok(value.map(encode_espace_storage_slot))
    }

    fn fetch_espace_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<Option<Box<[u8]>>> {
        let block_id = self.snapshot.espace_block_id.as_str();

        let code = self
            .provider
            .get_espace_code_at(block_id, address)
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        encode_espace_code(expected_code_hash, code)
            .map(Some)
            .map_err(|error| self.codec_error("encode_espace_code", error))
    }

    fn fetch_native_interest_rate(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let value = self
            .provider
            .get_native_interest_rate(epoch)
            .map_err(|error| self.provider_error("get_native_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_accumulate_interest_rate(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let value = self
            .provider
            .get_native_accumulate_interest_rate(epoch)
            .map_err(|error| self.provider_error("get_native_accumulate_interest_rate", error))?;

        Ok(Some(encode_native_u256(value)))
    }

    fn fetch_native_total_issued(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let supply_info = self
            .provider
            .get_native_supply_info(epoch)
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_issued)))
    }

    fn fetch_native_total_staking(&self) -> StorageResult<Option<Box<[u8]>>> {
        let epoch = self.snapshot.native_epoch.as_str();

        let supply_info = self
            .provider
            .get_native_supply_info(epoch)
            .map_err(|error| self.provider_error("get_native_supply_info", error))?;

        Ok(Some(encode_native_u256(supply_info.total_staking)))
    }

    fn provider_error(
        &self,
        operation: &'static str,
        error: RemoteStateProviderError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage provider error: operation={operation}, snapshot={:?},
              reason={error}",
            self.snapshot
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn codec_error(&self, operation: &'static str, error: StateValueCodecError) -> StorageError {
        let message = format!(
            "rpc-backed storage codec error: operation={operation}, snapshot={:?}, reason={error}",
            self.snapshot
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn lock_error(name: &'static str) -> StorageError {
        StorageError::Msg(format!("rpc-backed storage {name} mutex poisoned"))
    }
}

impl StorageStateTrait for RpcBackedStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
        let key_bytes = access_key.to_key_bytes();
        // Overlay writes shadow the remote snapshot for this simulation.
        if let Some(read) = self
            .overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .get(&key_bytes)
            .cloned()
        {
            return Ok(Self::cached_read_to_result(read));
        }

        let cache_key = self.cache_key(access_key);
        if let Some(read) = self
            .cache
            .lock()
            .map_err(|_| Self::lock_error("cache"))?
            .get(&cache_key)
            .cloned()
        {
            return Ok(Self::cached_read_to_result(read));
        }

        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let rpc_key = match StateReadRequest::from_storage_key(access_key) {
            Ok(rpc_key) => rpc_key,
            Err(error) => return Err(self.unsupported_storage_key("get", access_key, error)),
        };

        let value = self.fetch_rpc_value(&rpc_key)?;

        let cached_read = match &value {
            Some(bytes) => CachedRead::Present(bytes.clone()),
            None => CachedRead::Absent,
        };

        self.cache
            .lock()
            .map_err(|_| Self::lock_error("cache"))?
            .insert(cache_key, cached_read);

        Ok(value)
    }

    fn set(&mut self, access_key: StorageKeyWithSpace, value: Box<[u8]>) -> StorageResult<()> {
        self.overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .insert(access_key.to_key_bytes(), CachedRead::Present(value));
        Ok(())
    }

    fn delete(&mut self, access_key: StorageKeyWithSpace) -> StorageResult<()> {
        self.overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .insert(access_key.to_key_bytes(), CachedRead::Absent);
        Ok(())
    }

    fn delete_test_only(
        &mut self,
        access_key: StorageKeyWithSpace,
    ) -> StorageResult<Option<Box<[u8]>>> {
        Err(self.unsupported("delete_test_only", access_key))
    }

    fn delete_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(self.unsupported("delete_all", access_key_prefix))
    }

    fn read_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(self.unsupported("read_all", access_key_prefix))
    }

    fn read_all_with_callback(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
        _callback: &mut dyn FnMut(MptKeyValue),
        _only_account_key: bool,
    ) -> StorageResult<()> {
        Err(self.unsupported("read_all_with_callback", access_key_prefix))
    }

    fn compute_state_root(&mut self) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_operation("compute_state_root"))
    }

    fn get_state_root(&self) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_operation("get_state_root"))
    }

    fn commit(&mut self, epoch: EpochId) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_commit(epoch))
    }
}
