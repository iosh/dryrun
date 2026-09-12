use std::{cell::RefCell, sync::Arc};

use alloy_primitives::{Bytes, U256 as AlloyU256};
use cfx_executor::{
    executive::{
        ChargeCollateral, ExecutionError, ExecutionOutcome, ExecutiveContext, TransactOptions,
        TransactSettings,
    },
    machine::Machine,
    state::State,
};
use cfx_types::{Address, AddressSpaceUtil, AddressWithSpace, Space, U256};
use conflux_provider::{CoreAddress, Network};
use primitives::transaction::{Action, NativeTransaction, TypedNativeTransaction};
use tokio::runtime::Handle;

use crate::{
    execution::{PreparedTransactionExecution, build_conflux_state},
    primitive::u256_from_cfx,
    state::ConfluxStateSource,
};

use super::CoreSpaceStateAccessError;

const READ_CALL_GAS_LIMIT: u64 = 100_000;

pub struct CoreSpaceStateAccess {
    source: Arc<ConfluxStateSource>,
    initial: CoreSpaceStateReader,
    finalized: CoreSpaceStateReader,
}

impl CoreSpaceStateAccess {
    pub(super) fn new(
        source: Arc<ConfluxStateSource>,
        runtime_handle: Handle,
        finalized_state: State,
        machine: Arc<Machine>,
        prepared: &PreparedTransactionExecution,
        address_network: Network,
    ) -> Result<Self, CoreSpaceStateAccessError> {
        let initial_state = build_conflux_state(Arc::clone(&source), runtime_handle)
            .map_err(|source| CoreSpaceStateAccessError::Initialization { source })?;
        let context = Arc::new(CoreSpaceReadContext {
            machine,
            env: prepared.env.clone(),
            spec: prepared.spec.clone(),
            caller: prepared.transaction.sender().address,
            address_network,
        });
        Ok(Self {
            source,
            initial: CoreSpaceStateReader::new(initial_state, Arc::clone(&context)),
            finalized: CoreSpaceStateReader::new(finalized_state, context),
        })
    }

    pub const fn initial(&self) -> &CoreSpaceStateReader {
        &self.initial
    }

    pub const fn finalized(&self) -> &CoreSpaceStateReader {
        &self.finalized
    }

    pub fn initial_deposit_lots(
        &self,
        address: CoreAddress,
    ) -> Result<Vec<CoreSpaceDepositLot>, CoreSpaceStateAccessError> {
        let address = self.initial.validate_address(address)?;
        self.initial_deposit_list(address).map(|deposits| {
            deposits
                .into_iter()
                .map(|deposit| CoreSpaceDepositLot {
                    principal_amount: u256_from_cfx(deposit.amount),
                    deposit_block_number: u256_from_cfx(deposit.deposit_time),
                    accumulated_interest_rate: u256_from_cfx(deposit.accumulated_interest_rate),
                })
                .collect()
        })
    }

    pub fn initial_vote_locks(
        &self,
        address: CoreAddress,
    ) -> Result<Vec<CoreSpaceVoteLockInfo>, CoreSpaceStateAccessError> {
        let address = self.initial.validate_address(address)?;
        self.initial_vote_list(address).map(|votes| {
            votes
                .into_iter()
                .map(|vote| CoreSpaceVoteLockInfo {
                    locked_amount: u256_from_cfx(vote.amount),
                    unlock_block_number: u256_from_cfx(vote.unlock_block_number),
                })
                .collect()
        })
    }

    pub fn accumulated_interest_rate(&self) -> AlloyU256 {
        u256_from_cfx(self.source.accumulated_interest_rate())
    }

    pub(crate) fn initial_deposit_list(
        &self,
        address: Address,
    ) -> Result<Vec<primitives::DepositInfo>, CoreSpaceStateAccessError> {
        self.source
            .deposit_lists()
            .for_account(address)
            .map_err(|source| CoreSpaceStateAccessError::RecordedState {
                operation: "read the request-local initial Core Space deposit list",
                source,
            })
    }

    pub(crate) fn initial_vote_list(
        &self,
        address: Address,
    ) -> Result<Vec<primitives::VoteStakeInfo>, CoreSpaceStateAccessError> {
        self.source
            .vote_lists()
            .for_account(address)
            .map_err(|source| CoreSpaceStateAccessError::RecordedState {
                operation: "read the request-local initial Core Space vote list",
                source,
            })
    }

    pub(crate) fn raw_accumulated_interest_rate(&self) -> U256 {
        self.source.accumulated_interest_rate()
    }
}

struct CoreSpaceReadContext {
    machine: Arc<Machine>,
    env: cfx_vm_types::Env,
    spec: cfx_vm_types::Spec,
    caller: Address,
    address_network: Network,
}

pub struct CoreSpaceStateReader {
    state: RefCell<Option<State>>,
    context: Arc<CoreSpaceReadContext>,
}

impl CoreSpaceStateReader {
    fn new(state: State, context: Arc<CoreSpaceReadContext>) -> Self {
        Self {
            state: RefCell::new(Some(state)),
            context,
        }
    }

    pub(super) fn account(
        &self,
        address: Address,
    ) -> Result<CoreSpaceAccountState, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            let address_with_space = address.with_native_space();
            let exists = state
                .exists(&address_with_space)
                .map_err(|source| operation("read Core Space account existence", source))?;
            if !exists {
                return Ok(CoreSpaceAccountState {
                    exists: false,
                    balance: U256::zero(),
                    nonce: U256::zero(),
                    code: None,
                });
            }
            let balance = state
                .balance(&address_with_space)
                .map_err(|source| operation("read Core Space account balance", source))?;
            let nonce = state
                .nonce(&address_with_space)
                .map_err(|source| operation("read Core Space account nonce", source))?;
            let code = state
                .code(&address_with_space)
                .map_err(|source| operation("read Core Space account code", source))?
                .map(|code| Bytes::copy_from_slice(code.as_slice()));
            Ok(CoreSpaceAccountState {
                exists,
                balance,
                nonce,
                code,
            })
        })
    }

    pub fn native_balance(
        &self,
        address: CoreAddress,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.core_balance_raw(self.validate_address(address)?)
    }

    pub fn staking_balance(
        &self,
        address: CoreAddress,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.staking_balance_raw(self.validate_address(address)?)
    }

    pub fn gas_sponsor_balance(
        &self,
        contract: CoreAddress,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.gas_sponsor_balance_raw(self.validate_address(contract)?)
    }

    pub fn deposit_list_length(
        &self,
        address: CoreAddress,
    ) -> Result<usize, CoreSpaceStateAccessError> {
        self.deposit_list_length_raw(self.validate_address(address)?)
    }

    pub fn vote_lock_count(
        &self,
        address: CoreAddress,
    ) -> Result<usize, CoreSpaceStateAccessError> {
        self.vote_list_length_raw(self.validate_address(address)?)
    }

    pub fn locked_staking_balance_at(
        &self,
        address: CoreAddress,
        block_number: u64,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.locked_staking_balance_at_raw(self.validate_address(address)?, block_number)
    }

    pub(crate) fn core_balance_raw(
        &self,
        address: Address,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .balance(&address.with_native_space())
                .map(u256_from_cfx)
                .map_err(|source| operation("read Core Space account balance", source))
        })
    }

    pub(crate) fn staking_balance_raw(
        &self,
        address: Address,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .staking_balance(&address)
                .map(u256_from_cfx)
                .map_err(|source| operation("read Core Space staking balance", source))
        })
    }

    pub(crate) fn gas_sponsor_balance_raw(
        &self,
        contract: Address,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .sponsor_balance_for_gas(&contract)
                .map(u256_from_cfx)
                .map_err(|source| operation("read Core Space gas sponsor balance", source))
        })
    }

    pub fn total_issued(&self) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| Ok(u256_from_cfx(state.total_issued_tokens())))
    }

    pub fn total_staking(&self) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| Ok(u256_from_cfx(state.total_staking_tokens())))
    }

    pub(crate) fn deposit_list_length_raw(
        &self,
        address: Address,
    ) -> Result<usize, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .deposit_list_length(&address)
                .map_err(|source| operation("read Core Space deposit-list length", source))
        })
    }

    pub(crate) fn vote_list_length_raw(
        &self,
        address: Address,
    ) -> Result<usize, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .vote_stake_list_length(&address)
                .map_err(|source| operation("read Core Space vote-list length", source))
        })
    }

    pub(crate) fn locked_staking_balance_at_raw(
        &self,
        address: Address,
        block_number: u64,
    ) -> Result<AlloyU256, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .locked_staking_balance_at_block_number(&address, block_number)
                .map(u256_from_cfx)
                .map_err(|source| operation("read Core Space vote-lock balance", source))
        })
    }

    fn validate_address(&self, address: CoreAddress) -> Result<Address, CoreSpaceStateAccessError> {
        let actual = address.network();
        let expected = self.context.address_network;
        if actual != expected {
            return Err(CoreSpaceStateAccessError::AddressNetworkMismatch { expected, actual });
        }
        Ok(Address::from(address.bytes()))
    }

    pub(super) fn staking(
        &self,
        address: Address,
    ) -> Result<CoreSpaceStakingState, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            Ok(CoreSpaceStakingState {
                staking_balance: state
                    .staking_balance(&address)
                    .map_err(|source| operation("read Core Space staking balance", source))?,
                storage_collateral: state
                    .collateral_for_storage(&address)
                    .map_err(|source| operation("read Core Space storage collateral", source))?,
                pos_locked_staking: state
                    .pos_locked_staking(&address)
                    .map_err(|source| operation("read Core Space PoS locked staking", source))?,
                deposit_count: state
                    .deposit_list_length(&address)
                    .map_err(|source| operation("read Core Space deposit list", source))?,
                vote_lock_count: state
                    .vote_stake_list_length(&address)
                    .map_err(|source| operation("read Core Space vote-lock list", source))?,
            })
        })
    }

    pub(super) fn contract(
        &self,
        address: Address,
    ) -> Result<CoreSpaceContractState, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            let address_with_space = address.with_native_space();
            let exists = state
                .exists(&address_with_space)
                .map_err(|source| operation("read Core Space contract existence", source))?;
            if !exists {
                return Ok(CoreSpaceContractState {
                    exists: false,
                    admin: None,
                    code: None,
                });
            }
            let admin = state
                .admin(&address)
                .map_err(|source| operation("read Core Space contract admin", source))?;
            let code = state
                .code(&address_with_space)
                .map_err(|source| operation("read Core Space contract code", source))?
                .map(|code| Bytes::copy_from_slice(code.as_slice()));
            Ok(CoreSpaceContractState {
                exists,
                admin: Some(admin),
                code,
            })
        })
    }

    pub(super) fn sponsorship(
        &self,
        contract: Address,
        account: Address,
    ) -> Result<CoreSpaceSponsorshipState, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            Ok(CoreSpaceSponsorshipState {
                gas_sponsor: state
                    .sponsor_for_gas(&contract)
                    .map_err(|source| operation("read Core Space gas sponsor", source))?,
                gas_balance: state
                    .sponsor_balance_for_gas(&contract)
                    .map_err(|source| operation("read Core Space gas sponsor balance", source))?,
                gas_bound: state
                    .sponsor_gas_bound(&contract)
                    .map_err(|source| operation("read Core Space gas sponsorship bound", source))?,
                storage_sponsor: state
                    .sponsor_for_collateral(&contract)
                    .map_err(|source| operation("read Core Space storage sponsor", source))?,
                storage_balance: state.sponsor_balance_for_collateral(&contract).map_err(
                    |source| operation("read Core Space storage sponsor balance", source),
                )?,
                account_is_eligible: state
                    .check_contract_whitelist(&contract, &account)
                    .map_err(|source| {
                        operation("read Core Space sponsorship access rule", source)
                    })?,
            })
        })
    }

    pub(super) fn global_state(&self) -> Result<CoreSpaceGlobalState, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            Ok(CoreSpaceGlobalState {
                total_issued: state.total_issued_tokens(),
                total_staking: state.total_staking_tokens(),
                total_storage: state.total_storage_tokens(),
                total_espace_tokens: state.total_espace_tokens(),
                used_storage_points: state.used_storage_points(),
                converted_storage_points: state.converted_storage_points(),
                total_pos_staking: state.total_pos_staking_tokens(),
                distributable_pos_interest: state.distributable_pos_interest(),
                last_distribute_block: state.last_distribute_block(),
                pow_base_reward: state.pow_base_reward(),
                base_fee_share_proportion: state.get_base_price_prop(),
            })
        })
    }

    pub(super) fn storage_word(
        &self,
        address: AddressWithSpace,
        key: &[u8],
    ) -> Result<U256, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .storage_at(&address, key)
                .map_err(|source| operation("read Conflux storage", source))
        })
    }

    pub(super) fn code(
        &self,
        address: AddressWithSpace,
    ) -> Result<Option<Bytes>, CoreSpaceStateAccessError> {
        self.with_state(|state| {
            state
                .code(&address)
                .map(|code| code.map(|code| Bytes::copy_from_slice(code.as_slice())))
                .map_err(|source| operation("read Conflux contract code", source))
        })
    }

    pub(super) fn read_call(
        &self,
        target: Address,
        calldata: Bytes,
    ) -> Result<CoreSpaceReadCallOutcome, CoreSpaceStateAccessError> {
        let mut state_slot = self.state.borrow_mut();
        let state = state_slot
            .as_mut()
            .ok_or(CoreSpaceStateAccessError::Unavailable)?;
        match execute_isolated_read_call(state, &self.context, target, calldata) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                state_slot.take();
                Err(error)
            }
        }
    }

    fn with_state<T>(
        &self,
        read: impl FnOnce(&State) -> Result<T, CoreSpaceStateAccessError>,
    ) -> Result<T, CoreSpaceStateAccessError> {
        let state = self.state.borrow();
        let state = state
            .as_ref()
            .ok_or(CoreSpaceStateAccessError::Unavailable)?;
        read(state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpaceDepositLot {
    principal_amount: AlloyU256,
    deposit_block_number: AlloyU256,
    accumulated_interest_rate: AlloyU256,
}

impl CoreSpaceDepositLot {
    pub const fn principal_amount(self) -> AlloyU256 {
        self.principal_amount
    }

    pub const fn deposit_block_number(self) -> AlloyU256 {
        self.deposit_block_number
    }

    pub const fn accumulated_interest_rate(self) -> AlloyU256 {
        self.accumulated_interest_rate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpaceVoteLockInfo {
    locked_amount: AlloyU256,
    unlock_block_number: AlloyU256,
}

impl CoreSpaceVoteLockInfo {
    pub const fn locked_amount(self) -> AlloyU256 {
        self.locked_amount
    }

    pub const fn unlock_block_number(self) -> AlloyU256 {
        self.unlock_block_number
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoreSpaceAccountState {
    pub(super) exists: bool,
    pub(super) balance: U256,
    pub(super) nonce: U256,
    pub(super) code: Option<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoreSpaceStakingState {
    pub(super) staking_balance: U256,
    pub(super) storage_collateral: U256,
    pub(super) pos_locked_staking: U256,
    pub(super) deposit_count: usize,
    pub(super) vote_lock_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoreSpaceContractState {
    pub(super) exists: bool,
    pub(super) admin: Option<Address>,
    pub(super) code: Option<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoreSpaceSponsorshipState {
    pub(super) gas_sponsor: Option<Address>,
    pub(super) gas_balance: U256,
    pub(super) gas_bound: U256,
    pub(super) storage_sponsor: Option<Address>,
    pub(super) storage_balance: U256,
    pub(super) account_is_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoreSpaceGlobalState {
    pub(super) total_issued: U256,
    pub(super) total_staking: U256,
    pub(super) total_storage: U256,
    pub(super) total_espace_tokens: U256,
    pub(super) used_storage_points: U256,
    pub(super) converted_storage_points: U256,
    pub(super) total_pos_staking: U256,
    pub(super) distributable_pos_interest: U256,
    pub(super) last_distribute_block: u64,
    pub(super) pow_base_reward: U256,
    pub(super) base_fee_share_proportion: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CoreSpaceReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Failed,
}

fn execute_isolated_read_call(
    state: &mut State,
    context: &CoreSpaceReadContext,
    target: Address,
    calldata: Bytes,
) -> Result<CoreSpaceReadCallOutcome, CoreSpaceStateAccessError> {
    if !state.no_checkpoint() {
        return Err(CoreSpaceStateAccessError::ReadCall {
            details: "read call cannot run with an active state checkpoint".to_owned(),
        });
    }
    let sender = context.caller.with_native_space();
    let nonce = state
        .nonce(&sender)
        .map_err(|source| operation("read Core Space read-call nonce", source))?;
    let chain_id = context
        .env
        .chain_id
        .get(&Space::Native)
        .copied()
        .ok_or_else(|| CoreSpaceStateAccessError::ReadCall {
            details: "execution environment is missing the Core Space chain id".to_owned(),
        })?;
    let transaction = TypedNativeTransaction::Cip155(NativeTransaction {
        nonce,
        gas_price: U256::zero(),
        gas: U256::from(READ_CALL_GAS_LIMIT),
        action: Action::Call(target),
        value: U256::zero(),
        storage_limit: u64::MAX,
        epoch_height: context.env.epoch_height,
        chain_id,
        data: calldata.to_vec(),
    })
    .fake_sign_rpc(sender);
    let mut env = context.env.clone();
    env.gas_limit = U256::from(READ_CALL_GAS_LIMIT);
    env.transaction_hash = transaction.hash();

    let snapshot = state.save();
    let outcome = ExecutiveContext::new(state, &env, &context.machine, &context.spec)
        .transact(
            &transaction,
            TransactOptions {
                observer: (),
                settings: TransactSettings {
                    charge_collateral: ChargeCollateral::EstimateSender,
                    charge_gas: false,
                    check_base_price: false,
                    check_epoch_bound: false,
                    forbid_eoa_with_code: false,
                },
            },
        )
        .map_err(|source| operation("execute Core Space read call", source))?;

    let result = match outcome {
        ExecutionOutcome::Finished(executed) => {
            CoreSpaceReadCallOutcome::Success(Bytes::from(executed.output))
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(cfx_vm_types::Error::Reverted),
            executed,
        ) => CoreSpaceReadCallOutcome::Reverted(Bytes::from(executed.output)),
        ExecutionOutcome::ExecutionErrorBumpNonce(
            ExecutionError::VmError(cfx_vm_types::Error::StateDbError(error)),
            _,
        ) => {
            return Err(operation("execute Core Space read call", error.0));
        }
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _)
        | ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => CoreSpaceReadCallOutcome::Failed,
    };
    state.update_state_post_tx_execution(!context.spec.cip645.fix_eip1153);
    if !state.no_checkpoint() {
        return Err(CoreSpaceStateAccessError::ReadCall {
            details: "read call left an active state checkpoint".to_owned(),
        });
    }
    state.restore(snapshot);
    Ok(result)
}

fn operation(operation: &'static str, source: cfx_statedb::Error) -> CoreSpaceStateAccessError {
    CoreSpaceStateAccessError::Operation { operation, source }
}
