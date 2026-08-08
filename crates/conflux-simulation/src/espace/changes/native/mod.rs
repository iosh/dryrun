mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use contract_standards::legacy::{Position, StatePhase};
use simulation_changes::PositionedChange;

use crate::{ConfluxSimulationError, execution::ConfluxExecutionOutput};

use self::verification::{read_native_balances, verify_native_changes};

pub(crate) use verification::NativeBalances;

pub(crate) struct EspaceNativeAnalysis {
    operations: NativeOperations,
    execution_fee: U256,
    burnt_fee: Option<U256>,
}

impl EspaceNativeAnalysis {
    pub(crate) fn from_execution(
        execution: &ConfluxExecutionOutput,
    ) -> Result<Self, ConfluxSimulationError> {
        Ok(Self {
            operations: collection::collect_native_operations(&execution.observations)?,
            execution_fee: execution.common.fee,
            burnt_fee: execution.common.burnt_fee,
        })
    }

    pub(crate) fn read_state(
        &self,
        state: &State,
        phase: StatePhase,
    ) -> Result<NativeBalances, ConfluxSimulationError> {
        read_native_balances(state, phase, &self.operations)
    }

    pub(crate) fn verify(
        &self,
        before_balances: &NativeBalances,
        after_balances: &NativeBalances,
    ) -> Result<Vec<PositionedChange>, ConfluxSimulationError> {
        verify_native_changes(
            &self.operations,
            before_balances,
            after_balances,
            self.execution_fee,
            self.burnt_fee,
        )
    }
}

#[derive(Debug)]
pub(crate) struct NativeOperations {
    balance_accounts: Vec<Address>,
    operations: Vec<NativeOperation>,
}

impl NativeOperations {
    fn from_operations(operations: Vec<NativeOperation>) -> Self {
        let mut balance_accounts = BTreeSet::new();

        for operation in &operations {
            match operation {
                NativeOperation::AccountTransfer { from, to, .. } => {
                    balance_accounts.insert(*from);
                    balance_accounts.insert(*to);
                }
                NativeOperation::GasPrecharge { payer, .. } => {
                    balance_accounts.insert(*payer);
                }
                NativeOperation::GasRefund { recipient, .. } => {
                    balance_accounts.insert(*recipient);
                }
            }
        }

        Self {
            balance_accounts: balance_accounts.into_iter().collect(),
            operations,
        }
    }
}

#[derive(Debug)]
enum NativeOperation {
    AccountTransfer {
        position: Position,
        from: Address,
        to: Address,
        amount: U256,
    },
    GasPrecharge {
        payer: Address,
        amount: U256,
    },
    GasRefund {
        recipient: Address,
        amount: U256,
    },
}
