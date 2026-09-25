use alloy_primitives::{Log, U64, U128};
use serde::{Serialize, Serializer, ser::SerializeMap};
use simulation_core::{
    codec::{OutcomeRef, revert_diagnostic},
    error::{Diagnostic, ErrorCode},
};

use crate::{EvmBlockContext, EvmExecutionOutcome, EvmExecutionResult, EvmSuccessOutput};

impl Serialize for EvmBlockContext {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_map(Some(2))?;
        state.serialize_entry("blockNumber", &U64::from(self.number))?;
        state.serialize_entry("blockHash", &self.hash)?;
        state.end()
    }
}

impl Serialize for EvmExecutionResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut result = serializer.serialize_map(None)?;
        let fee = self.fee().execution_gas_fee();
        result.serialize_entry("gasUsed", &U64::from(self.gas().gas_used()))?;
        result.serialize_entry("effectiveGasPrice", &U128::from(fee.effective_gas_price()))?;
        result.serialize_entry("gasFee", &fee.charged_amount())?;
        if let Some(burnt) = fee.burnt_amount_if_applicable() {
            result.serialize_entry("burntGasFee", &burnt)?;
        }
        if let Some(blob) = self.fee().blob_gas_fee() {
            result.serialize_entry("blobGasUsed", &U64::from(blob.gas_used()))?;
            result.serialize_entry("blobGasPrice", &U128::from(blob.gas_price()))?;
            result.serialize_entry("blobGasFee", &blob.charged_amount())?;
        }
        result.end()
    }
}

impl Serialize for EvmExecutionOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let view: OutcomeRef<'_, EvmExecutionResult, EvmSuccessOutput, Log> = match self {
            Self::Success {
                result,
                output,
                logs,
                ..
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
            Self::Halted { result, reason } => OutcomeRef::Failed {
                result,
                error: Diagnostic::new(ErrorCode::TransactionHalted, reason.to_string()),
            },
        };
        view.serialize(serializer)
    }
}
