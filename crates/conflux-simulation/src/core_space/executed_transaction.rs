use std::collections::{BTreeSet, HashMap};

use alloy_primitives::{Bytes, U256};
use cfx_executor::{
    executive::ExecutionError, executive_observer::AddressPocket, machine::Machine,
};
use cfx_types::{Address, AddressSpaceUtil, AddressWithSpace, Space};
use cfx_vm_types::CallType;
use conflux_provider::{CoreAddress, Network};

use crate::{
    execution::{
        CommittedExecutionTrace, ConfluxExecutionOutcome, ConfluxExecutionOutput, FrameAction,
        FrameId, PreparedTransactionExecution, TraceEvent,
    },
    primitive::u256_from_cfx,
};

use super::{
    CoreSpaceExecutionError, CoreSpaceExecutionFailure, CoreSpaceResultIntegrationError,
    outcome::map_execution_failure,
};

#[derive(Debug)]
pub(super) enum CoreSpaceFinalStatus {
    Success,
    Reverted,
    Failed(CoreSpaceExecutionFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceExecutionStatus {
    Success,
    Reverted,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoreSpaceExecutionPosition(usize);

impl CoreSpaceExecutionPosition {
    pub const fn index(self) -> usize {
        self.0
    }

    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CoreSpaceFrameId(usize);

impl CoreSpaceFrameId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceExecutionSpace {
    Core,
    Espace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceCallKind {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceTransferPocket {
    CoreBalance(alloy_primitives::Address),
    EspaceBalance(alloy_primitives::Address),
    StakingBalance(alloy_primitives::Address),
    StorageCollateral(alloy_primitives::Address),
    SponsorBalanceForGas(alloy_primitives::Address),
    SponsorBalanceForStorage(alloy_primitives::Address),
    MintBurn,
    GasPayment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpaceCommittedInternalTransfer {
    position: CoreSpaceExecutionPosition,
    frame_id: Option<CoreSpaceFrameId>,
    space: CoreSpaceExecutionSpace,
    from: CoreSpaceTransferPocket,
    to: CoreSpaceTransferPocket,
    value: U256,
}

impl CoreSpaceCommittedInternalTransfer {
    pub const fn position(self) -> CoreSpaceExecutionPosition {
        self.position
    }

    pub const fn frame_id(self) -> Option<CoreSpaceFrameId> {
        self.frame_id
    }

    pub const fn space(self) -> CoreSpaceExecutionSpace {
        self.space
    }

    pub const fn from(self) -> CoreSpaceTransferPocket {
        self.from
    }

    pub const fn to(self) -> CoreSpaceTransferPocket {
        self.to
    }

    pub const fn value(self) -> U256 {
        self.value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceFrameAction<'a> {
    Call {
        kind: CoreSpaceCallKind,
        caller: alloy_primitives::Address,
        target: alloy_primitives::Address,
        code_address: alloy_primitives::Address,
        value: U256,
        calldata: &'a [u8],
    },
    Create {
        creator: alloy_primitives::Address,
        expected_address: alloy_primitives::Address,
        actual_address: alloy_primitives::Address,
        value: U256,
        init_code: &'a [u8],
    },
}

#[derive(Debug, Clone, Copy)]
pub struct CoreSpaceCommittedFrame<'a> {
    id: CoreSpaceFrameId,
    parent: Option<CoreSpaceFrameId>,
    position: CoreSpaceExecutionPosition,
    space: CoreSpaceExecutionSpace,
    action: CoreSpaceFrameAction<'a>,
}

impl<'a> CoreSpaceCommittedFrame<'a> {
    pub const fn id(self) -> CoreSpaceFrameId {
        self.id
    }

    pub const fn parent(self) -> Option<CoreSpaceFrameId> {
        self.parent
    }

    pub const fn position(self) -> CoreSpaceExecutionPosition {
        self.position
    }

    pub const fn space(self) -> CoreSpaceExecutionSpace {
        self.space
    }

    pub const fn action(self) -> CoreSpaceFrameAction<'a> {
        self.action
    }
}

/// Immutable facts retained from one finalized Core Space execution.
#[derive(Debug)]
pub struct CoreSpaceExecutedTransaction {
    pub(super) status: CoreSpaceFinalStatus,
    pub(super) sender: Address,
    pub(super) output: Bytes,
    pub(super) intrinsic_gas: u64,
    pub(super) gas_used: u64,
    pub(super) gas_charged: u64,
    pub(super) gas_fee: U256,
    pub(super) burnt_gas_fee: Option<U256>,
    pub(super) effective_gas_price: U256,
    pub(super) gas_sponsor_paid: bool,
    pub(super) storage_sponsor_paid: bool,
    pub(super) storage_collateralized: u64,
    pub(super) storage_collateralized_entries: Vec<primitives::receipt::StorageChange>,
    pub(super) storage_released: Vec<primitives::receipt::StorageChange>,
    pub(super) contracts_created: Vec<AddressWithSpace>,
    pub(super) committed_trace: CommittedExecutionTrace,
    pub(super) committed_logs: Vec<primitives::LogEntry>,
    pub(super) cip78a: bool,
    pub(super) cip78b: bool,
    pub(super) cip97: bool,
    pub(super) execution_block_number: u64,
    pub(super) address_network: Network,
    pub(super) transaction_recipient: Option<Address>,
    active_internal_contracts: BTreeSet<Address>,
}

impl CoreSpaceExecutedTransaction {
    pub(super) fn from_outcome(
        outcome: ConfluxExecutionOutcome,
        prepared: &PreparedTransactionExecution,
        machine: &Machine,
        transaction_sender: CoreAddress,
        transaction_recipient: Option<CoreAddress>,
    ) -> Result<Self, CoreSpaceExecutionError> {
        let (status, output) = match outcome {
            ConfluxExecutionOutcome::Success(output) => (CoreSpaceFinalStatus::Success, output),
            ConfluxExecutionOutcome::Failed { error, details } => {
                let status = match error {
                    ExecutionError::VmError(cfx_vm_types::Error::Reverted) => {
                        CoreSpaceFinalStatus::Reverted
                    }
                    failure => CoreSpaceFinalStatus::Failed(map_execution_failure(
                        failure,
                        transaction_sender.network(),
                    )?),
                };
                (status, details)
            }
            ConfluxExecutionOutcome::NotExecutedDrop(_)
            | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
                return Err(CoreSpaceResultIntegrationError::invalid_executor_output(
                    "a rejected Core Space transaction cannot produce finalized execution data",
                )
                .into());
            }
        };

        verify_exposed_frames(&output.trace)?;
        verify_committed_logs(&output.trace, &output.logs)?;
        verify_created_contracts(&output.trace, &output.contracts_created)?;
        verify_fee_settlement(&output)?;

        let storage_collateralized =
            match output.storage_collateralized.as_slice() {
                [] => 0,
                [change] => change.collaterals.as_u64(),
                entries => {
                    return Err(CoreSpaceResultIntegrationError::invalid_executor_output(format!(
                    "executor returned {} storage collateral entries in sender-estimation mode",
                    entries.len()
                ))
                .into());
                }
            };
        let base_price = prepared.env.base_gas_price[Space::Native];
        let transaction_gas_price = *prepared.transaction.gas_price();
        let effective_gas_price = if transaction_gas_price < base_price {
            transaction_gas_price
        } else {
            prepared.transaction.effective_gas_price(&base_price)
        };
        let active_internal_contracts = machine
            .internal_contracts()
            .iter()
            .filter_map(|(address, contract)| {
                contract.is_active(&prepared.spec).then_some(*address)
            })
            .collect();

        Ok(Self {
            status,
            sender: prepared.transaction.sender().address,
            output: output.common.output,
            intrinsic_gas: output.base_gas,
            gas_used: output.common.gas_used,
            gas_charged: output.common.gas_charged,
            gas_fee: output.common.fee,
            burnt_gas_fee: output.common.burnt_fee,
            effective_gas_price: u256_from_cfx(effective_gas_price),
            gas_sponsor_paid: output.gas_sponsor_paid,
            storage_sponsor_paid: output.storage_sponsor_paid,
            storage_collateralized,
            storage_collateralized_entries: output.storage_collateralized,
            storage_released: output.storage_released,
            contracts_created: output.contracts_created,
            committed_trace: output.trace,
            committed_logs: output.logs,
            cip78a: prepared.spec.cip78a,
            cip78b: prepared.spec.cip78b,
            cip97: prepared.spec.cip97,
            execution_block_number: prepared.env.number,
            address_network: transaction_sender.network(),
            transaction_recipient: transaction_recipient
                .map(|address| Address::from(address.bytes())),
            active_internal_contracts,
        })
    }

    pub const fn status(&self) -> CoreSpaceExecutionStatus {
        match &self.status {
            CoreSpaceFinalStatus::Success => CoreSpaceExecutionStatus::Success,
            CoreSpaceFinalStatus::Reverted => CoreSpaceExecutionStatus::Reverted,
            CoreSpaceFinalStatus::Failed(_) => CoreSpaceExecutionStatus::Failed,
        }
    }

    pub fn transaction_sender(&self) -> CoreAddress {
        CoreAddress::from_bytes(self.sender.0, self.address_network)
            .expect("executed Core Space sender retains a validated network")
    }

    pub fn transaction_recipient(&self) -> Option<CoreAddress> {
        self.transaction_recipient.map(|address| {
            CoreAddress::from_bytes(address.0, self.address_network)
                .expect("executed Core Space recipient retains a validated network")
        })
    }

    pub const fn execution_block_number(&self) -> u64 {
        self.execution_block_number
    }

    pub const fn gas_fee(&self) -> U256 {
        self.gas_fee
    }

    pub const fn burnt_gas_fee(&self) -> Option<U256> {
        self.burnt_gas_fee
    }

    pub const fn gas_sponsor_paid(&self) -> bool {
        self.gas_sponsor_paid
    }

    pub const fn cip97_active(&self) -> bool {
        self.cip97
    }

    pub fn frames(&self) -> impl Iterator<Item = CoreSpaceCommittedFrame<'_>> {
        self.committed_trace.events().iter().filter_map(|event| {
            let TraceEvent::FrameStart { position, frame_id } = event else {
                return None;
            };
            let frame = self
                .committed_trace
                .try_frame(*frame_id)
                .expect("committed frame-start references a committed frame");
            Some(CoreSpaceCommittedFrame {
                id: CoreSpaceFrameId(frame_id.index()),
                parent: frame
                    .parent_id
                    .map(|parent| CoreSpaceFrameId(parent.index())),
                position: CoreSpaceExecutionPosition::from_index(*position),
                space: execution_space(frame.space),
                action: frame_action(&frame.action),
            })
        })
    }

    pub fn internal_transfers(
        &self,
    ) -> impl Iterator<Item = CoreSpaceCommittedInternalTransfer> + '_ {
        self.committed_trace.events().iter().filter_map(|event| {
            let TraceEvent::InternalTransfer {
                position,
                frame_id,
                space,
                from,
                to,
                value,
            } = event
            else {
                return None;
            };
            Some(CoreSpaceCommittedInternalTransfer {
                position: CoreSpaceExecutionPosition::from_index(*position),
                frame_id: frame_id.map(|id| CoreSpaceFrameId(id.index())),
                space: execution_space(*space),
                from: transfer_pocket(*from),
                to: transfer_pocket(*to),
                value: u256_from_cfx(*value),
            })
        })
    }

    pub(crate) const fn trace(&self) -> &CommittedExecutionTrace {
        &self.committed_trace
    }

    pub(crate) fn is_active_internal_contract(&self, address: Address) -> bool {
        self.active_internal_contracts.contains(&address)
    }
}

fn execution_space(space: Space) -> CoreSpaceExecutionSpace {
    match space {
        Space::Native => CoreSpaceExecutionSpace::Core,
        Space::Ethereum => CoreSpaceExecutionSpace::Espace,
    }
}

fn call_kind(kind: CallType) -> CoreSpaceCallKind {
    match kind {
        CallType::Call => CoreSpaceCallKind::Call,
        CallType::CallCode => CoreSpaceCallKind::CallCode,
        CallType::DelegateCall => CoreSpaceCallKind::DelegateCall,
        CallType::StaticCall => CoreSpaceCallKind::StaticCall,
        CallType::None => unreachable!("validated committed call frame cannot have CallType::None"),
    }
}

fn frame_action(action: &FrameAction) -> CoreSpaceFrameAction<'_> {
    match action {
        FrameAction::Call {
            call_type,
            caller,
            target,
            code_address,
            transferred_value,
            calldata,
            ..
        } => CoreSpaceFrameAction::Call {
            kind: call_kind(*call_type),
            caller: alloy_primitives::Address::from_slice(caller.as_bytes()),
            target: alloy_primitives::Address::from_slice(target.as_bytes()),
            code_address: alloy_primitives::Address::from_slice(code_address.as_bytes()),
            value: u256_from_cfx(*transferred_value),
            calldata,
        },
        FrameAction::Create {
            creator,
            created_address,
            actual_created_address,
            value,
            init_code,
        } => CoreSpaceFrameAction::Create {
            creator: alloy_primitives::Address::from_slice(creator.as_bytes()),
            expected_address: alloy_primitives::Address::from_slice(created_address.as_bytes()),
            actual_address: alloy_primitives::Address::from_slice(
                actual_created_address
                    .as_ref()
                    .expect("validated committed create frame has an actual address")
                    .as_bytes(),
            ),
            value: u256_from_cfx(*value),
            init_code,
        },
    }
}

fn transfer_pocket(pocket: AddressPocket) -> CoreSpaceTransferPocket {
    let address = |value: Address| alloy_primitives::Address::from_slice(value.as_bytes());
    match pocket {
        AddressPocket::Balance(account) => match account.space {
            Space::Native => CoreSpaceTransferPocket::CoreBalance(address(account.address)),
            Space::Ethereum => CoreSpaceTransferPocket::EspaceBalance(address(account.address)),
        },
        AddressPocket::StakingBalance(account) => {
            CoreSpaceTransferPocket::StakingBalance(address(account))
        }
        AddressPocket::StorageCollateral(account) => {
            CoreSpaceTransferPocket::StorageCollateral(address(account))
        }
        AddressPocket::SponsorBalanceForGas(account) => {
            CoreSpaceTransferPocket::SponsorBalanceForGas(address(account))
        }
        AddressPocket::SponsorBalanceForStorage(account) => {
            CoreSpaceTransferPocket::SponsorBalanceForStorage(address(account))
        }
        AddressPocket::MintBurn => CoreSpaceTransferPocket::MintBurn,
        AddressPocket::GasPayment => CoreSpaceTransferPocket::GasPayment,
    }
}

fn verify_exposed_frames(
    trace: &CommittedExecutionTrace,
) -> Result<(), CoreSpaceResultIntegrationError> {
    for (frame_id, frame) in trace.frames() {
        match &frame.action {
            FrameAction::Call { call_type, .. } if *call_type == CallType::None => {
                return Err(integration_error(format!(
                    "committed Core Space call frame {} has no call kind",
                    frame_id.index()
                )));
            }
            FrameAction::Create {
                actual_created_address,
                ..
            } if actual_created_address.is_none() => {
                return Err(integration_error(format!(
                    "committed Core Space create frame {} has no actual address",
                    frame_id.index()
                )));
            }
            FrameAction::Call { .. } | FrameAction::Create { .. } => {}
        }
    }
    Ok(())
}

fn verify_committed_logs(
    trace: &CommittedExecutionTrace,
    logs: &[primitives::LogEntry],
) -> Result<(), CoreSpaceResultIntegrationError> {
    let trace_logs = trace.events().iter().filter_map(|event| match event {
        TraceEvent::Log {
            frame_id,
            address,
            topics,
            data,
            ..
        } => Some((frame_id, address, topics, data)),
        _ => None,
    });
    let count = trace_logs.clone().count();
    if count != logs.len() {
        return Err(integration_error(format!(
            "committed trace contains {count} logs but executor returned {}",
            logs.len()
        )));
    }

    for (index, ((frame_id, address, topics, data), log)) in trace_logs.zip(logs).enumerate() {
        let frame = trace
            .try_frame(*frame_id)
            .ok_or_else(|| missing_frame(*frame_id))?;
        if frame.space != log.space
            || *address != log.address
            || *topics != log.topics
            || data.as_slice() != log.data.as_slice()
        {
            return Err(integration_error(format!(
                "committed trace log {index} does not match executor output"
            )));
        }
    }
    Ok(())
}

fn verify_created_contracts(
    trace: &CommittedExecutionTrace,
    contracts: &[AddressWithSpace],
) -> Result<(), CoreSpaceResultIntegrationError> {
    let mut expected = HashMap::<AddressWithSpace, usize>::new();
    for (_, frame) in trace.frames() {
        if let FrameAction::Create {
            actual_created_address: Some(address),
            ..
        } = frame.action
        {
            *expected.entry(address.with_space(frame.space)).or_default() += 1;
        }
    }

    if expected.values().sum::<usize>() != contracts.len() {
        return Err(integration_error(format!(
            "executor returned {} created contracts for {} committed create frames",
            contracts.len(),
            expected.values().sum::<usize>()
        )));
    }
    for contract in contracts {
        let Some(count) = expected.get_mut(contract) else {
            return Err(integration_error(format!(
                "executor-reported created contract {contract:?} is absent from the committed trace"
            )));
        };
        if *count == 0 {
            return Err(integration_error(format!(
                "executor reported created contract {contract:?} too many times"
            )));
        }
        *count -= 1;
    }
    if let Some((contract, _)) = expected.into_iter().find(|(_, count)| *count != 0) {
        return Err(integration_error(format!(
            "committed created contract {contract:?} is absent from executor output"
        )));
    }
    Ok(())
}

fn verify_fee_settlement(
    output: &ConfluxExecutionOutput,
) -> Result<(), CoreSpaceResultIntegrationError> {
    let mut precharge = U256::ZERO;
    let mut refund = U256::ZERO;

    for event in output.trace.events() {
        let TraceEvent::InternalTransfer {
            from, to, value, ..
        } = event
        else {
            continue;
        };
        let amount = u256_from_cfx(*value);
        match (from, to) {
            (payer, AddressPocket::GasPayment) if is_core_fee_payer(payer) => {
                precharge = precharge.checked_add(amount).ok_or_else(|| {
                    integration_error("gas precharge accumulation overflowed U256")
                })?;
            }
            (AddressPocket::GasPayment, payer) if is_core_fee_payer(payer) => {
                refund = refund
                    .checked_add(amount)
                    .ok_or_else(|| integration_error("gas refund accumulation overflowed U256"))?;
            }
            _ => {}
        }
    }

    let settled = precharge
        .checked_sub(refund)
        .ok_or_else(|| integration_error("gas refund exceeds gas precharge"))?;
    if settled != output.common.fee {
        return Err(integration_error(format!(
            "committed transfers settle gas fee {settled}, executor reports {}",
            output.common.fee
        )));
    }
    Ok(())
}

fn is_core_fee_payer(pocket: &AddressPocket) -> bool {
    matches!(
        pocket,
        AddressPocket::Balance(address) if address.space == Space::Native
    ) || matches!(pocket, AddressPocket::SponsorBalanceForGas(_))
}

fn missing_frame(frame_id: FrameId) -> CoreSpaceResultIntegrationError {
    integration_error(format!(
        "committed trace references missing frame {}",
        frame_id.index()
    ))
}

fn integration_error(details: impl Into<String>) -> CoreSpaceResultIntegrationError {
    CoreSpaceResultIntegrationError::invalid_executor_output(details)
}
