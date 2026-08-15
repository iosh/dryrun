use alloy_primitives::Address;
use cfx_executor::{machine::Machine, state::State};
use contract_standards::{MetadataCall, MetadataValues};

use crate::{
    espace::{EspaceChangesError, EspaceCompleteTransaction},
    execution::PreparedTransactionExecution,
};

use super::read_call::{ReadCallOutcome, execute_read_call};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(crate) fn load_metadata(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    transaction: &EspaceCompleteTransaction,
    calls: Vec<MetadataCall<Address>>,
) -> Result<MetadataValues<Address>, EspaceChangesError> {
    let mut values = MetadataValues::default();

    for (index, call) in calls.into_iter().enumerate() {
        if index >= MAX_METADATA_CALLS {
            values.record_unavailable(call);
            continue;
        }

        let outcome = execute_read_call(
            state,
            machine,
            prepared_execution,
            transaction.from,
            *call.contract_address(),
            call.call_data(),
            &call,
        )?;
        match outcome {
            ReadCallOutcome::Success(output) if output.len() <= MAX_METADATA_OUTPUT_BYTES => {
                values.record_output(call, &output);
            }
            ReadCallOutcome::Success(_) | ReadCallOutcome::Reverted | ReadCallOutcome::Failed => {
                values.record_unavailable(call);
            }
        }
    }

    Ok(values)
}
