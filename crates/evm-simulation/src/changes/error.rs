use alloy::primitives::{Address, U256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmNativeChangeError {
    #[error("native account {address} is missing from transaction state")]
    AccountMissing { address: Address },

    #[error(
        "native balance underflow for {address}: \
         balance {balance}, cannot subtract {amount}"
    )]
    BalanceUnderflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance overflow for {address}: \
         balance {balance}, cannot add {amount}"
    )]
    BalanceOverflow {
        address: Address,
        balance: U256,
        amount: U256,
    },

    #[error(
        "native balance mismatch for {address}: \
         replayed {replayed_balance}, transaction state {state_balance}"
    )]
    BalanceMismatch {
        address: Address,
        replayed_balance: U256,
        state_balance: U256,
    },
}
