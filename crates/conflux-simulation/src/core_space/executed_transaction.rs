use std::collections::HashMap;

use alloy_primitives::{Bytes, U256};
use cfx_executor::{executive::ExecutionError, executive_observer::AddressPocket};
use cfx_types::{Address, AddressSpaceUtil, AddressWithSpace, Space};

use crate::{
    execution::{
        CommittedExecutionTrace, ConfluxExecutionOutcome, ConfluxExecutionOutput, FrameAction,
        FrameId, PreparedTransactionExecution, TraceEvent,
    },
    primitive::u256_from_cfx,
};

use super::{CoreSpaceExecutionError, CoreSpaceResultIntegrationError};

#[derive(Debug)]
pub(super) enum CoreSpaceFinalStatus {
    Success,
    Reverted,
    Failed(ExecutionError),
}

/// Immutable facts retained from one finalized Core Space execution.
#[derive(Debug)]
pub(crate) struct CoreSpaceExecutedTransaction {
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
}

impl CoreSpaceExecutedTransaction {
    pub(super) fn from_outcome(
        outcome: ConfluxExecutionOutcome,
        prepared: &PreparedTransactionExecution,
    ) -> Result<Self, CoreSpaceExecutionError> {
        let (status, output) = match outcome {
            ConfluxExecutionOutcome::Success(output) => (CoreSpaceFinalStatus::Success, output),
            ConfluxExecutionOutcome::Failed { error, details } => {
                let status = if matches!(
                    error,
                    ExecutionError::VmError(cfx_vm_types::Error::Reverted)
                ) {
                    CoreSpaceFinalStatus::Reverted
                } else {
                    CoreSpaceFinalStatus::Failed(error)
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
        })
    }
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
