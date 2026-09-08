use alloy_primitives::Address;
use contract_standards::{
    DecodedStandardLog, Erc20Metadata, MetadataCall, MetadataValues, StandardChange,
};

use crate::espace::{EspaceChangesError, EspaceReadCallOutcome, EspaceStateReader};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(super) struct TokenMetadataOutcomes {
    values: MetadataValues<Address>,
}

impl TokenMetadataOutcomes {
    pub(super) fn erc20(&self, contract: &Address) -> Erc20Metadata {
        self.values
            .erc20_metadata(contract)
            .unwrap_or_else(|_| unreachable!("token metadata collection records every outcome"))
    }

    pub(super) fn standard_change(
        &self,
        decoded: DecodedStandardLog<Address>,
    ) -> StandardChange<Address> {
        decoded
            .into_change(&self.values)
            .unwrap_or_else(|_| unreachable!("token metadata collection records every outcome"))
    }
}

pub(super) fn load_metadata(
    state: &EspaceStateReader,
    calls: Vec<MetadataCall<Address>>,
) -> Result<TokenMetadataOutcomes, EspaceChangesError> {
    let mut values = MetadataValues::default();

    for (index, call) in calls.into_iter().enumerate() {
        if index >= MAX_METADATA_CALLS {
            values.record_unavailable(call);
            continue;
        }

        let outcome = state
            .read_call(*call.contract_address(), call.call_data())
            .map_err(|error| EspaceChangesError::StateAccess {
                details: format!("metadata probe {call:?}: {error}"),
            })?;
        match outcome {
            EspaceReadCallOutcome::Success(output) if output.len() <= MAX_METADATA_OUTPUT_BYTES => {
                values.record_output(call, &output);
            }
            EspaceReadCallOutcome::Success(_)
            | EspaceReadCallOutcome::Reverted(_)
            | EspaceReadCallOutcome::Failed => {
                values.record_unavailable(call);
            }
        }
    }

    Ok(TokenMetadataOutcomes { values })
}
