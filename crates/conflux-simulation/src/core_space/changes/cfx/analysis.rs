use alloy_primitives::U256;
use cfx_executor::{machine::Machine, state::State};
use contract_standards::StatePhase;

use super::{
    CfxBalanceLocation, CfxOperations, CfxStateValues, StakingBalanceEffects,
    collect_cfx_operations, determine_gas_fee_payer, read_cfx_state_values, verify_cfx_changes,
};
use crate::{
    ConfluxSimulationError,
    core_space::changes::PositionedCoreSpaceChange,
    execution::{ConfluxTransactionExecution, TransactionExecutionOutcome},
    state::MaskedSponsorWhitelistEntries,
};

#[derive(Debug)]
pub(crate) struct CfxAnalysisInput {
    expected_gas_fee_payer: CfxBalanceLocation,
    operations: CfxOperations,
    staking_balance_effects: StakingBalanceEffects,
    execution_fee: U256,
    burnt_fee: Option<U256>,
}

impl CfxAnalysisInput {
    pub(crate) fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        masked_sponsor_whitelist_entries: &MaskedSponsorWhitelistEntries,
    ) -> Result<Self, ConfluxSimulationError> {
        let TransactionExecutionOutcome::Success(details) = &execution.outcome else {
            return Err(ConfluxSimulationError::ExecutionInternal {
                message: "successful execution is required for Core Space CFX analysis".into(),
            });
        };

        let expected_gas_fee_payer =
            determine_gas_fee_payer(&execution.prepared.transaction, details.gas_sponsor_paid)?;
        let operations = collect_cfx_operations(
            &details.observations,
            &details.contracts_created,
            &details.storage_released,
            machine,
            &execution.prepared.spec,
        )?;
        operations
            .reject_masked_sponsorship_access_dependencies(masked_sponsor_whitelist_entries)?;
        let staking_balance_effects = operations.staking_balance_effects();

        Ok(Self {
            expected_gas_fee_payer,
            operations,
            staking_balance_effects,
            execution_fee: details.common.fee,
            burnt_fee: details.common.burnt_fee,
        })
    }

    pub(crate) fn read_state(
        &self,
        state: &State,
        phase: StatePhase,
    ) -> Result<CfxStateValues, ConfluxSimulationError> {
        read_cfx_state_values(state, phase, &self.operations)
    }

    pub(crate) fn verify(
        &self,
        before_state: &CfxStateValues,
        after_state: &CfxStateValues,
    ) -> Result<Vec<PositionedCoreSpaceChange>, ConfluxSimulationError> {
        verify_cfx_changes(
            &self.operations,
            before_state,
            after_state,
            self.expected_gas_fee_payer,
            self.execution_fee,
            self.burnt_fee,
        )
    }

    pub(crate) fn staking_balance_effects(&self) -> &StakingBalanceEffects {
        &self.staking_balance_effects
    }
}
