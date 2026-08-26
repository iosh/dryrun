use alloy::primitives::Address;
use contract_standards::{MetadataCall, MetadataValues};

use crate::{
    EvmChangesError,
    state::{EvmReadCallResult, EvmStateReadError, EvmStateView},
};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(crate) fn load_metadata(
    after: &EvmStateView,
    calls: Vec<MetadataCall<Address>>,
) -> Result<MetadataValues<Address>, EvmChangesError> {
    let mut values = MetadataValues::default();

    for (index, call) in calls.into_iter().enumerate() {
        if index >= MAX_METADATA_CALLS {
            values.record_unavailable(call);
            continue;
        }

        let outcome = match after.read_call(*call.contract_address(), call.call_data()) {
            Ok(outcome) => outcome,
            Err(EvmStateReadError::StateAccess(source)) => {
                return Err(EvmChangesError::MetadataStateAccess {
                    call: call.clone(),
                    source,
                });
            }
            Err(EvmStateReadError::ReadCallExecution { details }) => {
                return Err(EvmChangesError::MetadataProbeExecution {
                    call: call.clone(),
                    details,
                });
            }
        };

        match outcome {
            EvmReadCallResult::Success(output) if output.len() <= MAX_METADATA_OUTPUT_BYTES => {
                values.record_output(call, &output);
            }
            EvmReadCallResult::Success(_)
            | EvmReadCallResult::Reverted(_)
            | EvmReadCallResult::Halted(_) => {
                values.record_unavailable(call);
            }
        }
    }

    Ok(values)
}
