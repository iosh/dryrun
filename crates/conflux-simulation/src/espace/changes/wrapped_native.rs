use alloy::{
    primitives::{Address, U256},
    sol,
    sol_types::SolEvent,
};
use simulation_core::observation::LogFilter;

sol! {
    event Deposit(address indexed account, uint256 amount);
    event Withdrawal(address indexed account, uint256 amount);
}

#[derive(Debug, Clone, Copy)]
pub(super) enum WrappedNativeEvent {
    Deposit { account: Address, raw_amount: U256 },
    Withdrawal { account: Address, raw_amount: U256 },
}

pub(super) fn checkpoint_filters(contract_address: Address) -> Vec<LogFilter> {
    let mut filters = Vec::new();
    for topic0 in [Deposit::SIGNATURE_HASH, Withdrawal::SIGNATURE_HASH] {
        filters.push(LogFilter {
            address: Some(contract_address),
            topic0,
        });
    }
    filters
}

pub(super) fn decode_wrapped_native_log(
    topics: &[alloy::primitives::B256],
    data: &[u8],
) -> Result<Option<WrappedNativeEvent>, &'static str> {
    let Some(topic0) = topics.first() else {
        return Ok(None);
    };
    if *topic0 != Deposit::SIGNATURE_HASH && *topic0 != Withdrawal::SIGNATURE_HASH {
        return Ok(None);
    }
    if topics.len() != 2 || data.len() != 32 {
        return Err("malformed wrapped-native event");
    }
    if *topic0 == Deposit::SIGNATURE_HASH {
        let event = Deposit::decode_raw_log_validate(topics.iter().copied(), data)
            .map_err(|_| "malformed wrapped-native Deposit event")?;
        Ok(Some(WrappedNativeEvent::Deposit {
            account: event.account,
            raw_amount: event.amount,
        }))
    } else {
        let event = Withdrawal::decode_raw_log_validate(topics.iter().copied(), data)
            .map_err(|_| "malformed wrapped-native Withdrawal event")?;
        Ok(Some(WrappedNativeEvent::Withdrawal {
            account: event.account,
            raw_amount: event.amount,
        }))
    }
}
