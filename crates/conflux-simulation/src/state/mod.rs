use crate::Error;
use cfx_executor::state::State;

mod core_space_internal;
mod reader;
mod state_item;
mod state_value_encoding;
mod storage;
pub(crate) use core_space_internal::{
    CoreSpaceInternalStateItem, parse_core_space_internal_storage,
};
pub(crate) use reader::StateSource;
pub(crate) use storage::new_state;

pub(crate) fn read_account(
    state: &State,
    address: &cfx_types::AddressWithSpace,
) -> Result<Option<primitives::Account>, Error> {
    // exists loads the account through the normal state cache and propagates DB errors.
    state.exists(address)?;
    let cache = state.cache.read();
    let entry = cache.get(address).ok_or(Error::Internal(
        "loaded account is absent from the state cache",
    ))?;
    Ok(entry.account().map(|account| account.as_account()))
}
