use alloy::primitives::{Address, U256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmNativeChangeError {
    #[error("native analysis could not access the execution transition")]
    TransitionUnavailable,

    #[error(
        "selfdestruct-to-self at observation {observation_index} burns \
         {amount} from {contract}, which cannot be represented"
    )]
    UnsupportedSelfDestructToSelf {
        observation_index: usize,
        contract: Address,
        amount: U256,
    },

    #[error("native account {address} is missing from transaction state")]
    NativeAccountMissing { address: Address },

    #[error(
        "native balance underflow for {address}: \
         balance {balance}, cannot subtract {amount}"
    )]
    NativeBalanceUnderflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance overflow for {address}: \
         balance {balance}, cannot add {amount}"
    )]
    NativeBalanceOverflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance mismatch for {address}: \
         replayed {replayed_balance}, transaction state {state_balance}"
    )]
    NativeBalanceMismatch {
        address: Address,
        replayed_balance: U256,
        state_balance: U256,
    },
}
