//! Representation rules shared by Ethereum-format transaction requests.
//!
//! `TransactionRequest` can hold conflicting fields, and Alloy's builders choose
//! a type from those fields without enforcing the explicit RPC `type`. Check the
//! request before conversion can discard fields. Nonce, balance and fee validity
//! remain the VM's responsibility.

use alloy_consensus::TxType;
use alloy_primitives::{Address, TxKind};
use alloy_rpc_types_eth::TransactionRequest;

/// Checks request representation before any chain state is requested.
/// Returns the required sender and the optional explicit type constraint.
pub fn parse_transaction_input(
    request: &TransactionRequest,
) -> Result<(Address, Option<TxType>), String> {
    let sender = request.from.ok_or_else(|| "from is required".to_owned())?;
    let explicit_type = request
        .transaction_type
        .map(|value| {
            TxType::try_from(value).map_err(|_| format!("unsupported transaction type {value:#x}"))
        })
        .transpose()?;
    let tx_type = explicit_type.unwrap_or_else(|| request.minimal_tx_type());
    check_type_compatibility(request, tx_type)?;
    request
        .input
        .unique_input()
        .map_err(|error| error.to_string())?;
    Ok((sender, explicit_type))
}

/// Computes an automatic fee cap without truncation or saturation.
pub fn fee_cap(base_fee: u128, priority_fee: u128, multiplier: u128) -> Option<u128> {
    base_fee.checked_mul(multiplier)?.checked_add(priority_fee)
}

/// Rejects fields that cannot be represented by the chosen transaction type.
fn check_type_compatibility(request: &TransactionRequest, tx_type: TxType) -> Result<(), String> {
    if matches!(tx_type, TxType::Legacy | TxType::Eip2930) {
        if request.max_fee_per_gas.is_some() || request.max_priority_fee_per_gas.is_some() {
            return Err("dynamic fee fields require a dynamic fee transaction type".into());
        }
    } else if request.gas_price.is_some() {
        return Err("gasPrice requires a legacy or EIP-2930 transaction".into());
    }
    if tx_type == TxType::Legacy && request.access_list.is_some() {
        return Err("accessList is not a field of a legacy transaction".into());
    }
    if tx_type != TxType::Eip4844
        && (request.has_eip4844_blob_data() || request.max_fee_per_blob_gas.is_some())
    {
        return Err("blob fields require an EIP-4844 transaction".into());
    }
    if tx_type != TxType::Eip7702 && request.authorization_list.is_some() {
        return Err("authorizationList requires an EIP-7702 transaction".into());
    }
    if matches!(tx_type, TxType::Eip4844 | TxType::Eip7702)
        && !matches!(request.to, Some(TxKind::Call(_)))
    {
        return Err(format!(
            "transaction type {:#x} cannot create a contract",
            tx_type as u8
        ));
    }
    Ok(())
}
