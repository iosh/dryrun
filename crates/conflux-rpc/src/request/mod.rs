mod core_space;
mod espace;

use cfx_types::U256;

use crate::error::ValidationError;

pub(crate) use core_space::SimulateCoreSpaceTransactionRequest;
pub(crate) use espace::SimulateEspaceTransactionRequest;

fn chain_id_from_wire(chain_id: U256) -> Result<u32, ValidationError> {
    u32::try_from(chain_id).map_err(|_| {
        ValidationError::invalid_params(
            "`transaction.chainId` must fit into an unsigned 32-bit integer",
        )
    })
}
