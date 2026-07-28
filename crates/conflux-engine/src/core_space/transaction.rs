use cfx_types::Address;
use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};

use crate::{ConfluxTransaction, ConfluxTransactionVariant, execution::CoreSpaceTransactionInput};

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
) -> CoreSpaceTransactionInput {
    let sender = input.transaction.body.from;
    let tx = build_typed_core_space_transaction(input);

    CoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(input: CoreSpaceTransaction) -> TypedNativeTransaction {
    let CoreSpaceTransaction {
        transaction,
        storage_limit,
        epoch_height,
    } = input;
    let ConfluxTransaction { body, gas_limit } = transaction;
    let crate::ConfluxTransactionBody {
        to,
        nonce,
        value,
        data,
        chain_id,
        variant,
        ..
    } = body;

    let action = action_from_to(to);

    match variant {
        CoreSpaceTransactionVariant::Legacy { gas_price } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price,
                gas: gas_limit,
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
            gas_price,
            gas: gas_limit,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list,
        }),
        CoreSpaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas: gas_limit,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list,
        }),
    }
}

fn action_from_to(to: Option<Address>) -> Action {
    to.map_or(Action::Create, Action::Call)
}
