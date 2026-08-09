use alloy::primitives::Address;
use contract_standards::{MetadataCall, MetadataValues};
use revm::context_interface::result::EVMError;

use crate::{CompleteTransaction, EvmChangesError, EvmExecutionObserver, execution::MainnetEvm};

use super::read_call::{ReadCallOutcome, execute_read_call, with_read_call_context};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(crate) fn load_metadata(
    evm: &mut MainnetEvm<EvmExecutionObserver>,
    transaction: &CompleteTransaction,
    chain_id: u64,
    calls: Vec<MetadataCall<Address>>,
) -> Result<MetadataValues<Address>, EvmChangesError> {
    with_read_call_context(evm, |evm| {
        let mut values = MetadataValues::default();

        for (index, call) in calls.into_iter().enumerate() {
            if index >= MAX_METADATA_CALLS {
                values.record_unavailable(call);
                continue;
            }

            let outcome = execute_read_call(
                evm,
                transaction,
                chain_id,
                *call.contract_address(),
                call.call_data(),
            )
            .map_err(|error| match error {
                EVMError::Database(source) => EvmChangesError::MetadataStateAccess {
                    call: call.clone(),
                    source: source.into(),
                },
                error => EvmChangesError::MetadataProbeExecution {
                    call: call.clone(),
                    details: error.to_string(),
                },
            })?;

            match outcome {
                ReadCallOutcome::Success(output) if output.len() <= MAX_METADATA_OUTPUT_BYTES => {
                    values.record_output(call, &output);
                }
                ReadCallOutcome::Success(_)
                | ReadCallOutcome::Reverted
                | ReadCallOutcome::Halted => values.record_unavailable(call),
            }
        }

        Ok(values)
    })
}
