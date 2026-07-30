use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};

use crate::{
    ConfluxTransaction, ConfluxTransactionVariant,
    execution::CoreSpaceTransactionInput,
    primitive::{access_list_to_cfx, address_to_cfx, u256_to_cfx},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceEpochRef {
    LatestState,
    Number(u64),
}

pub type CoreSpaceTransactionVariant = ConfluxTransactionVariant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransaction {
    pub transaction: ConfluxTransaction,
    pub storage_limit: u64,
    pub epoch_height: u64,
}

pub(crate) fn build_core_space_transaction_input(
    input: CoreSpaceTransaction,
    chain_id: u32,
) -> CoreSpaceTransactionInput {
    let sender = address_to_cfx(input.transaction.from);
    let tx = build_typed_core_space_transaction(input, chain_id);

    CoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(
    input: CoreSpaceTransaction,
    chain_id: u32,
) -> TypedNativeTransaction {
    let CoreSpaceTransaction {
        transaction,
        storage_limit,
        epoch_height,
    } = input;
    let ConfluxTransaction {
        to,
        nonce,
        gas_limit,
        value,
        data,
        variant,
        ..
    } = transaction;

    let action = to.map_or(Action::Create, |address| {
        Action::Call(address_to_cfx(address))
    });
    let nonce = nonce.into();
    let gas = gas_limit.into();
    let value = u256_to_cfx(value);
    let data = data.to_vec();

    match variant {
        CoreSpaceTransactionVariant::Legacy { gas_price } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price: gas_price.into(),
                gas,
                action,
                value,
                storage_limit,
                epoch_height,
                chain_id,
                data,
            })
        }
        CoreSpaceTransactionVariant::AccessList {
            gas_price,
            access_list,
        } => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price: gas_price.into(),
            gas,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
        CoreSpaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas: max_priority_fee_per_gas.into(),
            max_fee_per_gas: max_fee_per_gas.into(),
            gas,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
    }
}
