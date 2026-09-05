use std::collections::{HashMap, HashSet};

use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_executor::{executive::ExecutionError, executive_observer::AddressPocket};
use cfx_types::Space;
use cfx_vm_types::Error as VmError;

use crate::{
    execution::{
        CommittedExecutionTrace, ConfluxExecutionOutcome, ConfluxExecutionOutput, FrameAction,
        FrameId, TraceEvent,
    },
    primitive::{address_from_cfx, b256_from_cfx, u256_from_cfx},
};

use super::{EspaceExecutionError, EspaceResultIntegrationError};

/// A stable position in one finalized eSpace execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EspaceExecutionPosition(usize);

impl EspaceExecutionPosition {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A stable identifier for a committed eSpace frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EspaceFrameId(usize);

impl EspaceFrameId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EspaceExecutionSpace {
    Espace,
    Core,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EspaceCallKind {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceFrameAction {
    Call {
        kind: EspaceCallKind,
        caller: Address,
        target: Address,
        code_address: Address,
        value: U256,
        calldata: Bytes,
    },
    Create {
        creator: Address,
        expected_address: Address,
        actual_address: Address,
        value: U256,
        init_code: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceCommittedFrame {
    id: EspaceFrameId,
    parent: Option<EspaceFrameId>,
    position: EspaceExecutionPosition,
    space: EspaceExecutionSpace,
    action: EspaceFrameAction,
}

impl EspaceCommittedFrame {
    pub const fn id(&self) -> EspaceFrameId {
        self.id
    }

    pub const fn parent(&self) -> Option<EspaceFrameId> {
        self.parent
    }

    pub const fn position(&self) -> EspaceExecutionPosition {
        self.position
    }

    pub const fn space(&self) -> EspaceExecutionSpace {
        self.space
    }

    pub const fn action(&self) -> &EspaceFrameAction {
        &self.action
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceCommittedLog {
    position: EspaceExecutionPosition,
    frame_id: EspaceFrameId,
    space: EspaceExecutionSpace,
    address: Address,
    topics: Vec<B256>,
    data: Bytes,
}

impl EspaceCommittedLog {
    pub const fn position(&self) -> EspaceExecutionPosition {
        self.position
    }

    pub const fn frame_id(&self) -> EspaceFrameId {
        self.frame_id
    }

    pub const fn space(&self) -> EspaceExecutionSpace {
        self.space
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub fn topics(&self) -> &[B256] {
        &self.topics
    }

    pub const fn data(&self) -> &Bytes {
        &self.data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EspaceTransferPocket {
    EspaceBalance(Address),
    CoreBalance(Address),
    StakingBalance(Address),
    StorageCollateral(Address),
    SponsorBalanceForGas(Address),
    SponsorBalanceForStorage(Address),
    MintBurn,
    GasPayment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EspaceCommittedInternalTransfer {
    position: EspaceExecutionPosition,
    frame_id: Option<EspaceFrameId>,
    space: EspaceExecutionSpace,
    from: EspaceTransferPocket,
    to: EspaceTransferPocket,
    value: U256,
}

impl EspaceCommittedInternalTransfer {
    pub const fn position(self) -> EspaceExecutionPosition {
        self.position
    }

    pub const fn frame_id(self) -> Option<EspaceFrameId> {
        self.frame_id
    }

    pub const fn space(self) -> EspaceExecutionSpace {
        self.space
    }

    pub const fn from(self) -> EspaceTransferPocket {
        self.from
    }

    pub const fn to(self) -> EspaceTransferPocket {
        self.to
    }

    pub const fn value(self) -> U256 {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceStorageChange {
    address: Address,
    collaterals: u64,
}

impl EspaceStorageChange {
    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn collaterals(&self) -> u64 {
        self.collaterals
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EspaceContractAddress {
    space: EspaceExecutionSpace,
    address: Address,
}

impl EspaceContractAddress {
    pub const fn space(self) -> EspaceExecutionSpace {
        self.space
    }

    pub const fn address(self) -> Address {
        self.address
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceExecutionStatus {
    Success,
    Reverted,
    Failed,
}

/// Immutable data produced after one eSpace transaction reaches its finalized
/// execution boundary. It contains no mutable State or provider handle.
#[derive(Debug)]
pub struct EspaceExecutedTransaction {
    status: EspaceExecutionStatus,
    committed_frames: Vec<EspaceCommittedFrame>,
    committed_logs: Vec<EspaceCommittedLog>,
    internal_transfers: Vec<EspaceCommittedInternalTransfer>,
    storage_collateralized: Vec<EspaceStorageChange>,
    storage_released: Vec<EspaceStorageChange>,
    contracts_created: Vec<EspaceContractAddress>,
}

impl EspaceExecutedTransaction {
    pub(crate) fn from_outcome(
        outcome: &ConfluxExecutionOutcome,
    ) -> Result<Self, EspaceExecutionError> {
        let (status, output) = match outcome {
            ConfluxExecutionOutcome::Success(output) => (EspaceExecutionStatus::Success, output),
            ConfluxExecutionOutcome::Failed { error, details } => {
                let status = if matches!(error, ExecutionError::VmError(VmError::Reverted)) {
                    EspaceExecutionStatus::Reverted
                } else {
                    EspaceExecutionStatus::Failed
                };
                (status, details)
            }
            ConfluxExecutionOutcome::NotExecutedDrop(_)
            | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
                return Err(EspaceResultIntegrationError::invalid_executor_output(
                    "a rejected eSpace transaction cannot produce finalized executed-transaction data",
                )
                .into());
            }
        };

        verify_fee_settlement(output).map_err(EspaceExecutionError::from)?;
        Self::from_output(output, status).map_err(EspaceExecutionError::from)
    }

    fn from_output(
        output: &ConfluxExecutionOutput,
        status: EspaceExecutionStatus,
    ) -> Result<Self, EspaceResultIntegrationError> {
        verify_committed_logs(&output.trace, &output.logs)?;
        let committed_frames = convert_frames(&output.trace)?;
        let (committed_logs, internal_transfers) =
            convert_events(&output.trace, &committed_frames)?;
        let contracts_created = convert_created_contracts(&output, &committed_frames)?;
        let storage_collateralized = output
            .storage_collateralized
            .iter()
            .map(convert_storage_change)
            .collect();
        let storage_released = output
            .storage_released
            .iter()
            .map(convert_storage_change)
            .collect();

        Ok(Self {
            status,
            committed_frames,
            committed_logs,
            internal_transfers,
            storage_collateralized,
            storage_released,
            contracts_created,
        })
    }

    pub fn status(&self) -> EspaceExecutionStatus {
        self.status
    }

    pub fn is_success(&self) -> bool {
        self.status == EspaceExecutionStatus::Success
    }

    pub fn committed_frames(&self) -> &[EspaceCommittedFrame] {
        &self.committed_frames
    }

    pub fn committed_logs(&self) -> &[EspaceCommittedLog] {
        &self.committed_logs
    }

    pub fn internal_transfers(&self) -> &[EspaceCommittedInternalTransfer] {
        &self.internal_transfers
    }

    pub fn storage_collateralized(&self) -> &[EspaceStorageChange] {
        &self.storage_collateralized
    }

    pub fn storage_released(&self) -> &[EspaceStorageChange] {
        &self.storage_released
    }

    pub fn contracts_created(&self) -> &[EspaceContractAddress] {
        &self.contracts_created
    }
}

fn verify_committed_logs(
    trace: &CommittedExecutionTrace,
    logs: &[primitives::LogEntry],
) -> Result<(), EspaceResultIntegrationError> {
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
    let trace_count = trace_logs.clone().count();
    if trace_count != logs.len() {
        return Err(integration_error(format!(
            "trace contains {trace_count} committed logs but executor returned {} logs",
            logs.len()
        )));
    }

    for (index, ((frame_id, address, topics, data), log)) in trace_logs.zip(logs).enumerate() {
        let Some(frame) = trace.try_frame(*frame_id) else {
            return Err(missing_frame(frame_id.index()));
        };
        if *address != log.address
            || *topics != log.topics
            || data.as_slice() != log.data.as_slice()
            || frame.space != log.space
        {
            return Err(integration_error(format!(
                "committed trace log {index} does not match executor log"
            )));
        }
    }
    Ok(())
}

fn convert_frames(
    trace: &CommittedExecutionTrace,
) -> Result<Vec<EspaceCommittedFrame>, EspaceResultIntegrationError> {
    let frame_ids = trace.frames().map(|(id, _)| id).collect::<HashSet<_>>();
    let mut frame_positions = HashMap::new();
    let mut frame_starts = HashSet::new();
    let mut last_position = None;
    for event in trace.events() {
        let position = event.position();
        if let Some(last) = last_position
            && position <= last
        {
            return Err(integration_error(
                "committed trace event positions are not strictly increasing",
            ));
        }
        last_position = Some(position);
        match event {
            TraceEvent::FrameStart { frame_id, .. } => {
                if !frame_ids.contains(frame_id) {
                    return Err(missing_frame(frame_id.index()));
                }
                if !frame_starts.insert(*frame_id) {
                    return Err(integration_error(format!(
                        "committed frame {} has more than one frame-start event",
                        frame_id.index()
                    )));
                }
                frame_positions.insert(*frame_id, EspaceExecutionPosition(position));
            }
            TraceEvent::Log { frame_id, .. } => {
                if !frame_ids.contains(frame_id) {
                    return Err(missing_frame(frame_id.index()));
                }
            }
            TraceEvent::InternalTransfer { frame_id, .. } => {
                if let Some(frame_id) = frame_id
                    && !frame_ids.contains(frame_id)
                {
                    return Err(missing_frame(frame_id.index()));
                }
            }
        }
    }

    let mut frames = Vec::new();
    for (id, frame) in trace.frames() {
        let position = frame_positions.get(&id).copied().ok_or_else(|| {
            integration_error(format!(
                "committed frame {} has no frame-start position",
                id.index()
            ))
        })?;
        let parent = frame.parent_id.map(|parent| {
            if frame_ids.contains(&parent) {
                Ok(EspaceFrameId(parent.index()))
            } else {
                Err(missing_frame(parent.index()))
            }
        });
        let parent = match parent {
            Some(result) => Some(result?),
            None => None,
        };
        let action = match &frame.action {
            FrameAction::Call {
                call_type,
                caller,
                target,
                code_address,
                transferred_value,
                calldata_len,
                calldata,
                calldata_prefix,
            } => {
                if *calldata_len != calldata.len() || calldata_prefix.len() > calldata.len() {
                    return Err(integration_error(format!(
                        "committed frame {} has an invalid calldata length",
                        id.index()
                    )));
                }
                EspaceFrameAction::Call {
                    kind: match call_type {
                        cfx_vm_types::CallType::Call => EspaceCallKind::Call,
                        cfx_vm_types::CallType::CallCode => EspaceCallKind::CallCode,
                        cfx_vm_types::CallType::DelegateCall => EspaceCallKind::DelegateCall,
                        cfx_vm_types::CallType::StaticCall => EspaceCallKind::StaticCall,
                        cfx_vm_types::CallType::None => {
                            return Err(EspaceResultIntegrationError::invalid_executor_output(
                                format!("committed call frame {} has CallType::None", id.index()),
                            ));
                        }
                    },
                    caller: address_from_cfx(*caller),
                    target: address_from_cfx(*target),
                    code_address: address_from_cfx(*code_address),
                    value: u256_from_cfx(*transferred_value),
                    calldata: Bytes::copy_from_slice(calldata),
                }
            }
            FrameAction::Create {
                creator,
                created_address,
                actual_created_address,
                value,
                init_code,
            } => EspaceFrameAction::Create {
                creator: address_from_cfx(*creator),
                expected_address: address_from_cfx(*created_address),
                actual_address: actual_created_address
                    .map(address_from_cfx)
                    .ok_or_else(|| {
                        integration_error(format!(
                            "successful CREATE frame {} has no actual created address",
                            id.index()
                        ))
                    })?,
                value: u256_from_cfx(*value),
                init_code: Bytes::copy_from_slice(init_code),
            },
        };
        frames.push(EspaceCommittedFrame {
            id: EspaceFrameId(id.index()),
            parent,
            position,
            space: map_space(frame.space),
            action,
        });
    }
    Ok(frames)
}

fn convert_events(
    trace: &CommittedExecutionTrace,
    frames: &[EspaceCommittedFrame],
) -> Result<
    (
        Vec<EspaceCommittedLog>,
        Vec<EspaceCommittedInternalTransfer>,
    ),
    EspaceResultIntegrationError,
> {
    let frame_ids = frames.iter().map(|frame| frame.id).collect::<HashSet<_>>();
    let mut logs = Vec::new();
    let mut transfers = Vec::new();
    for event in trace.events() {
        match event {
            TraceEvent::FrameStart { .. } => {}
            TraceEvent::Log {
                position,
                frame_id,
                address,
                topics,
                data,
            } => {
                ensure_frame(*frame_id, &frame_ids)?;
                let frame = trace
                    .try_frame(*frame_id)
                    .ok_or_else(|| missing_frame(frame_id.index()))?;
                logs.push(EspaceCommittedLog {
                    position: EspaceExecutionPosition(*position),
                    frame_id: EspaceFrameId(frame_id.index()),
                    space: map_space(frame.space),
                    address: address_from_cfx(*address),
                    topics: topics.iter().copied().map(b256_from_cfx).collect(),
                    data: Bytes::copy_from_slice(data),
                });
            }
            TraceEvent::InternalTransfer {
                position,
                frame_id,
                space,
                from,
                to,
                value,
            } => {
                if let Some(frame_id) = frame_id {
                    ensure_frame(*frame_id, &frame_ids)?;
                }
                transfers.push(EspaceCommittedInternalTransfer {
                    position: EspaceExecutionPosition(*position),
                    frame_id: frame_id.map(|id| EspaceFrameId(id.index())),
                    space: map_space(*space),
                    from: map_pocket(from),
                    to: map_pocket(to),
                    value: u256_from_cfx(*value),
                });
            }
        }
    }
    Ok((logs, transfers))
}

fn ensure_frame(
    frame_id: FrameId,
    frame_ids: &HashSet<EspaceFrameId>,
) -> Result<(), EspaceResultIntegrationError> {
    if frame_ids.contains(&EspaceFrameId(frame_id.index())) {
        Ok(())
    } else {
        Err(missing_frame(frame_id.index()))
    }
}

fn convert_created_contracts(
    output: &ConfluxExecutionOutput,
    frames: &[EspaceCommittedFrame],
) -> Result<Vec<EspaceContractAddress>, EspaceResultIntegrationError> {
    let created = frames
        .iter()
        .filter_map(|frame| match &frame.action {
            EspaceFrameAction::Create { actual_address, .. } => Some(EspaceContractAddress {
                space: frame.space,
                address: *actual_address,
            }),
            EspaceFrameAction::Call { .. } => None,
        })
        .collect::<Vec<_>>();
    let contracts = output
        .contracts_created
        .iter()
        .map(|address| EspaceContractAddress {
            space: map_space(address.space),
            address: address_from_cfx(address.address),
        })
        .collect::<Vec<_>>();
    if created.len() != contracts.len() {
        return Err(integration_error(format!(
            "executor returned {} contracts-created entries for {} committed CREATE frames",
            contracts.len(),
            created.len()
        )));
    }

    let mut expected_counts = HashMap::<EspaceContractAddress, usize>::new();
    for contract in created {
        *expected_counts.entry(contract).or_default() += 1;
    }
    for contract in &contracts {
        let Some(count) = expected_counts.get_mut(contract) else {
            return Err(integration_error(format!(
                "executor-reported created contract {contract:?} is absent from committed CREATE frames"
            )));
        };
        if *count == 0 {
            return Err(integration_error(format!(
                "executor reported created contract {contract:?} more times than committed CREATE frames"
            )));
        }
        *count -= 1;
    }
    if let Some((address, _)) = expected_counts.into_iter().find(|(_, count)| *count != 0) {
        return Err(integration_error(format!(
            "committed CREATE contract {address:?} is absent from executor contracts-created"
        )));
    }
    Ok(contracts)
}

fn convert_storage_change(change: &primitives::receipt::StorageChange) -> EspaceStorageChange {
    EspaceStorageChange {
        address: address_from_cfx(change.address),
        collaterals: change.collaterals.as_u64(),
    }
}

fn map_space(space: Space) -> EspaceExecutionSpace {
    match space {
        Space::Ethereum => EspaceExecutionSpace::Espace,
        Space::Native => EspaceExecutionSpace::Core,
    }
}

fn map_pocket(pocket: &AddressPocket) -> EspaceTransferPocket {
    match pocket {
        AddressPocket::Balance(address) if address.space == Space::Ethereum => {
            EspaceTransferPocket::EspaceBalance(address_from_cfx(address.address))
        }
        AddressPocket::Balance(address) => {
            EspaceTransferPocket::CoreBalance(address_from_cfx(address.address))
        }
        AddressPocket::StakingBalance(address) => {
            EspaceTransferPocket::StakingBalance(address_from_cfx(*address))
        }
        AddressPocket::StorageCollateral(address) => {
            EspaceTransferPocket::StorageCollateral(address_from_cfx(*address))
        }
        AddressPocket::SponsorBalanceForGas(address) => {
            EspaceTransferPocket::SponsorBalanceForGas(address_from_cfx(*address))
        }
        AddressPocket::SponsorBalanceForStorage(address) => {
            EspaceTransferPocket::SponsorBalanceForStorage(address_from_cfx(*address))
        }
        AddressPocket::MintBurn => EspaceTransferPocket::MintBurn,
        AddressPocket::GasPayment => EspaceTransferPocket::GasPayment,
    }
}

fn integration_error(details: impl Into<String>) -> EspaceResultIntegrationError {
    EspaceResultIntegrationError::invalid_executor_output(details)
}

fn missing_frame(frame: usize) -> EspaceResultIntegrationError {
    integration_error(format!("committed trace references missing frame {frame}"))
}

fn verify_fee_settlement(
    output: &ConfluxExecutionOutput,
) -> Result<(), EspaceResultIntegrationError> {
    super::settlement::verify_fee_settlement(output)
}
