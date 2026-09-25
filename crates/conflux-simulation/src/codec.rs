use alloy_primitives::U64;
use serde::{Serialize, Serializer, ser::SerializeMap};
use simulation_core::{
    codec::{OutcomeRef, revert_diagnostic},
    error::{Diagnostic, ErrorCode},
};

use crate::{core_space::*, espace::*};

impl Serialize for EspaceBlockContext {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_map(Some(2))?;
        state.serialize_entry("blockNumber", &U64::from(self.number))?;
        state.serialize_entry("blockHash", &self.hash)?;
        state.end()
    }
}
impl Serialize for CoreSpaceBlockContext {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_map(Some(2))?;
        state.serialize_entry("epochNumber", &U64::from(self.epoch_number))?;
        state.serialize_entry("pivotHash", &self.pivot_hash)?;
        state.end()
    }
}
impl Serialize for EspaceExecutionResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut result = serializer.serialize_map(None)?;
        result.serialize_entry("gasUsed", &U64::from(self.gas().gas_used()))?;
        result.serialize_entry("gasFee", &self.fee().charged_amount())?;
        if let Some(burnt) = self.fee().burnt_amount() {
            result.serialize_entry("burntGasFee", &burnt)?;
        }
        result.end()
    }
}
impl Serialize for CoreSpaceExecutionResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut result = serializer.serialize_map(None)?;
        result.serialize_entry("gasUsed", &U64::from(self.gas().gas_used()))?;
        result.serialize_entry("gasFee", &self.gas_fee())?;
        if let Some(burnt) = self.burnt_gas_fee() {
            result.serialize_entry("burntGasFee", &burnt)?;
        }
        result.serialize_entry("effectiveGasPrice", &self.effective_gas_price())?;
        result.serialize_entry("gasCoveredBySponsor", &self.gas_covered_by_sponsor())?;
        result.serialize_entry(
            "storageCollateralized",
            &U64::from(self.storage_collateralized()),
        )?;
        result.serialize_entry(
            "storageCoveredBySponsor",
            &self.storage_covered_by_sponsor(),
        )?;
        result.end()
    }
}
impl Serialize for EspaceExecutionOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view: OutcomeRef<'_, EspaceExecutionResult, EspaceSuccessOutput, EspaceLog> = match self
        {
            Self::Success {
                result,
                output,
                logs,
            } => OutcomeRef::Success {
                result,
                output,
                logs,
            },
            Self::Reverted {
                result,
                revert_data,
                reason,
            } => OutcomeRef::Reverted {
                result,
                revert_data,
                error: revert_diagnostic(reason.as_ref()),
            },
            Self::Failed { result, failure } => OutcomeRef::Failed {
                result,
                error: Diagnostic::new(ErrorCode::TransactionHalted, failure.to_string()),
            },
        };
        view.serialize(serializer)
    }
}
impl Serialize for CoreSpaceExecutionOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view: OutcomeRef<'_, CoreSpaceExecutionResult, CoreSpaceSuccessOutput, CoreSpaceLog> =
            match self {
                Self::Success {
                    result,
                    output,
                    logs,
                } => OutcomeRef::Success {
                    result,
                    output,
                    logs,
                },
                Self::Reverted {
                    result,
                    revert_data,
                    reason,
                } => OutcomeRef::Reverted {
                    result,
                    revert_data,
                    error: revert_diagnostic(reason.as_ref()),
                },
                Self::Failed { result, failure } => OutcomeRef::Failed {
                    result,
                    error: Diagnostic::new(ErrorCode::TransactionHalted, failure.to_string()),
                },
            };
        view.serialize(serializer)
    }
}
