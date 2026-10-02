use super::{
    state_item::{CoreSpaceStateItem as Core, EspaceStateItem as Eth, StateItem},
    state_value_encoding::*,
};
use crate::{StateError, anchor::Anchor, primitive::*};
use alloy::{
    primitives::{Address as AlloyAddress, U256 as AlloyU256},
    providers::{DynProvider, Provider},
};
use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_types::{Address, U256};
use conflux_provider::{BlockHashOrEpochNumber, ConfluxProvider, CoreAddress, Network};
use simulation_core::{Limits, ReadBudget};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

type Cache = HashMap<StateItem, Option<Box<[u8]>>>;

/// One remote cache and budget for preparation, execution and both views.
pub(crate) struct StateSource {
    pub anchor: Anchor,
    pub core: ConfluxProvider,
    pub espace: DynProvider,
    pub budget: ReadBudget,
    network: Network,
    cache: Mutex<Cache>,
}

impl StateSource {
    pub async fn new(
        anchor: Anchor,
        core: ConfluxProvider,
        espace: DynProvider,
        network: Network,
        limits: Limits,
    ) -> Result<Self, StateError> {
        let source = Self {
            anchor,
            core,
            espace,
            network,
            budget: ReadBudget::new(limits),
            cache: Mutex::new(HashMap::new()),
        };
        source.load_globals().await?;
        Ok(source)
    }

    async fn load_globals(&self) -> Result<(), StateError> {
        let epoch = self.anchor.epoch();
        let mut batch = self.core.batch();
        let interest = batch.cfx_get_interest_rate(epoch)?;
        let accumulated = batch.cfx_get_accumulate_interest_rate(epoch)?;
        let supply = batch.cfx_get_supply_info(epoch)?;
        let collateral = batch.cfx_get_collateral_info(epoch)?;
        let pos = batch.cfx_get_pos_economics(epoch)?;
        let vote = batch.cfx_get_params_from_vote(epoch)?;
        let burnt = batch.cfx_get_fee_burnt(epoch)?;
        // Each decoded global is a separate state item even when its RPC is batched.
        for _ in 0..14 {
            self.budget.record_state_read()?;
        }
        batch.send().await?;
        let (interest, accumulated, supply, collateral, pos, vote, burnt) =
            tokio::try_join!(interest, accumulated, supply, collateral, pos, vote, burnt)?;
        let units = |v| {
            u256_to_cfx(v)
                .checked_mul(*DRIPS_PER_STORAGE_COLLATERAL_UNIT)
                .ok_or_else(|| {
                    StateError::Unavailable("global storage collateral overflows U256".into())
                })
        };
        let globals = [
            (Core::InterestRate, u256_to_cfx(interest)),
            (Core::AccumulateInterestRate, u256_to_cfx(accumulated)),
            (Core::TotalIssued, u256_to_cfx(supply.total_issued)),
            (Core::TotalStaking, u256_to_cfx(supply.total_staking)),
            (Core::TotalEvmToken, u256_to_cfx(supply.total_espace_tokens)),
            (Core::TotalStorage, u256_to_cfx(supply.total_collateral)),
            (
                Core::UsedStoragePoints,
                units(collateral.used_storage_points)?,
            ),
            (
                Core::ConvertedStoragePoints,
                units(collateral.converted_storage_points)?,
            ),
            (
                Core::TotalPosStaking,
                u256_to_cfx(pos.total_pos_staking_tokens),
            ),
            (
                Core::DistributablePosInterest,
                u256_to_cfx(pos.distributable_pos_interest),
            ),
            (
                Core::LastDistributeBlock,
                u256_to_cfx(pos.last_distribute_block),
            ),
            (Core::PowBaseReward, u256_to_cfx(vote.pow_base_reward)),
            (Core::TotalBurnt1559, u256_to_cfx(burnt)),
            (Core::BaseFeeProp, u256_to_cfx(vote.base_fee_share_prop)),
        ];
        self.cache
            .lock()
            .await
            .extend(globals.into_iter().map(|(key, value)| {
                (
                    StateItem::CoreSpace(key),
                    Some(encode_core_space_u256(value)),
                )
            }));
        Ok(())
    }

    pub async fn read(&self, item: StateItem) -> Result<Option<Box<[u8]>>, StateError> {
        // Hold the async lock through a miss so concurrent views cannot refetch.
        let mut cache = self.cache.lock().await;
        if let Some(value) = cache.get(&item) {
            return Ok(value.clone());
        }
        self.budget.record_state_read()?;
        let value = match item {
            StateItem::Espace(Eth::Account { address }) => {
                self.load_espace_account(address, &mut cache).await?
            }
            StateItem::Espace(Eth::StorageSlot { address, slot }) => {
                let value = self
                    .espace
                    .get_storage_at(
                        address_from_cfx(address),
                        AlloyU256::from_be_slice(slot.as_bytes()),
                    )
                    .block_id(self.anchor.block())
                    .await
                    .map_err(|source| StateError::EspaceProvider {
                        operation: "eth_getStorageAt",
                        source,
                    })?;
                (!value.is_zero()).then(|| encode_storage_slot(u256_to_cfx(value)))
            }
            StateItem::Espace(Eth::Code { .. }) => {
                return Err(StateError::Unavailable(
                    "code was not loaded with its eSpace account".into(),
                ));
            }
            StateItem::CoreSpace(item) => self.load_core(item).await?,
        };
        cache.insert(item, value.clone());
        Ok(value)
    }

    pub async fn nonce(&self, address: AlloyAddress) -> Result<u64, StateError> {
        let value = self
            .read(StateItem::Espace(Eth::Account {
                address: address_to_cfx(address),
            }))
            .await?;
        let Some(value) = value else {
            return Ok(0);
        };
        let account: primitives::account::EthereumAccount = rlp::decode(&value).map_err(invalid)?;
        account
            .nonce
            .try_into()
            .map_err(|_| StateError::Unavailable("account nonce exceeds u64".into()))
    }

    async fn load_espace_account(
        &self,
        address: Address,
        cache: &mut Cache,
    ) -> Result<Option<Box<[u8]>>, StateError> {
        let rpc_address = address_from_cfx(address);
        let block = self.anchor.block();
        let (balance, nonce, code) = tokio::try_join!(
            async {
                self.espace
                    .get_balance(rpc_address)
                    .block_id(block)
                    .await
                    .map_err(|source| StateError::EspaceProvider {
                        operation: "eth_getBalance",
                        source,
                    })
            },
            async {
                self.espace
                    .get_transaction_count(rpc_address)
                    .block_id(block)
                    .await
                    .map_err(|source| StateError::EspaceProvider {
                        operation: "eth_getTransactionCount",
                        source,
                    })
            },
            async {
                self.espace
                    .get_code_at(rpc_address)
                    .block_id(block)
                    .await
                    .map_err(|source| StateError::EspaceProvider {
                        operation: "eth_getCode",
                        source,
                    })
            },
        )?;
        let hash = keccak_hash::keccak(&code);
        let code_value = if code.is_empty() {
            None
        } else {
            Some(encode_code(hash, Address::zero(), Arc::new(code.to_vec())).map_err(invalid)?)
        };
        cache.insert(
            StateItem::Espace(Eth::Code {
                address,
                code_hash: hash,
            }),
            code_value,
        );
        Ok(encode_espace_account(
            u256_to_cfx(balance),
            nonce.into(),
            &code,
        ))
    }

    fn core_address(&self, address: Address) -> Result<CoreAddress, StateError> {
        CoreAddress::from_bytes(address.0, self.network).map_err(invalid)
    }

    fn pivot(&self) -> BlockHashOrEpochNumber {
        BlockHashOrEpochNumber::BlockHash {
            hash: self.anchor.pivot_hash,
            require_pivot: Some(true),
        }
    }

    async fn load_core(&self, item: Core) -> Result<Option<Box<[u8]>>, StateError> {
        let epoch = self.anchor.epoch();
        match item {
            Core::Account { address } => {
                let rpc_address = self.core_address(address)?;
                let mut batch = self.core.batch();
                let account = batch.cfx_get_account(rpc_address, epoch)?;
                let collateral = batch.cfx_get_collateral_for_storage(rpc_address, epoch)?;
                batch.send().await?;
                let (account, collateral) = tokio::try_join!(account, collateral)?;
                let collateral = u256_to_cfx(collateral);
                let points = used_storage_point_collateral(
                    u256_to_cfx(account.collateral_for_storage),
                    collateral,
                )
                .map_err(invalid)?;
                if should_encode_core_space_contract_account(
                    address,
                    b256_to_cfx(account.code_hash),
                ) {
                    let sponsor = self.core.cfx_get_sponsor_info(rpc_address, epoch).await?;
                    encode_core_space_contract_account(&account, collateral, points, sponsor)
                        .map_err(invalid)
                } else {
                    if !points.is_zero() {
                        return Err(StateError::Unavailable(
                            "basic account has storage-point collateral".into(),
                        ));
                    }
                    Ok(encode_core_space_basic_account(
                        u256_to_cfx(account.balance),
                        u256_to_cfx(account.nonce),
                        u256_to_cfx(account.staking_balance),
                        collateral,
                        u256_to_cfx(account.accumulated_interest_return),
                    ))
                }
            }
            Core::Code { address, code_hash } => {
                let code = self
                    .core
                    .cfx_get_code(self.core_address(address)?, self.pivot())
                    .await?;
                if code.is_empty() && code_hash == keccak_hash::KECCAK_EMPTY {
                    return Ok(None);
                }
                encode_code(code_hash, address, Arc::new(code.to_vec()))
                    .map(Some)
                    .map_err(invalid)
            }
            Core::StorageSlot { address, slot } => {
                let value = self
                    .core
                    .cfx_get_storage_at(
                        self.core_address(address)?,
                        AlloyU256::from_be_slice(slot.as_bytes()),
                        Some(self.pivot()),
                    )
                    .await?;
                Ok(value.map(|v| encode_storage_slot(U256::from_big_endian(v.as_slice()))))
            }
            Core::DepositList { address } => {
                let values = self
                    .core
                    .cfx_get_deposit_list(self.core_address(address)?, epoch)
                    .await?;
                let values = values
                    .into_iter()
                    .map(|v| primitives::DepositInfo {
                        amount: u256_to_cfx(v.amount),
                        deposit_time: u256_to_cfx(v.deposit_time),
                        accumulated_interest_rate: u256_to_cfx(v.accumulated_interest_rate),
                    })
                    .collect();
                Ok(encode_core_space_deposit_list(values))
            }
            Core::VoteList { address } => {
                let values = self
                    .core
                    .cfx_get_vote_list(self.core_address(address)?, epoch)
                    .await?;
                let values = values
                    .into_iter()
                    .map(|v| primitives::VoteStakeInfo {
                        amount: u256_to_cfx(v.amount),
                        unlock_block_number: u256_to_cfx(v.unlock_block_number),
                    })
                    .collect();
                Ok(encode_core_space_vote_list(values))
            }
            Core::InternalContractStorage(_) => Err(StateError::Unavailable(
                "raw sponsor whitelist state is not available through Core RPC".into(),
            )),
            _ => Err(StateError::Unavailable(
                "global state was not loaded".into(),
            )),
        }
    }
}

fn invalid(error: impl std::fmt::Display) -> StateError {
    StateError::Unavailable(error.to_string())
}
