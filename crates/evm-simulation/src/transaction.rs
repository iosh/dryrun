use alloy::{
    consensus::TxType,
    primitives::{Address, TxKind},
    rpc::types::TransactionRequest,
};
use alloy_evm::EvmEnv;
use revm::{
    context::{Block, Cfg, TxEnv},
    context_interface::either::Either,
};

use crate::Error;

/// Fills omitted fields with the defaults of `eth_call`, so that the request
/// echoed in the result is the transaction that ran.
///
/// - `type`: inferred from supplied fields; conflicts are checked by [`tx_env`].
/// - `gas`: the block gas limit, capped by the per-transaction limit.
/// - `nonce`: the sender's nonce in the state.
/// - `chainId`: the chain's id.
/// - fees: zero; see [`skips_fee_checks`].
/// - blob hashes: those of the sidecar's blobs.
/// - blob fee: the block's blob gas price.
pub(crate) fn fill_defaults(
    request: &mut TransactionRequest,
    env: &EvmEnv,
    state_nonce: impl FnOnce() -> Result<u64, Error>,
) -> Result<(), Error> {
    if request.transaction_type.is_none() {
        request.transaction_type = Some(request.minimal_tx_type() as u8);
    }
    if request.gas.is_none() {
        let cap = env.cfg_env.tx_gas_limit_cap();
        request.gas = Some(env.block_env.gas_limit.min(cap));
    }
    if request.nonce.is_none() {
        request.nonce = Some(state_nonce()?);
    }
    if request.chain_id.is_none() {
        request.chain_id = Some(env.cfg_env.chain_id);
    }
    if request.blob_versioned_hashes.is_none() {
        request.populate_blob_hashes();
    }
    if request.has_eip4844_blob_data() && request.max_fee_per_blob_gas.is_none() {
        request.max_fee_per_blob_gas = env.block_env.blob_gasprice();
    }
    Ok(())
}

/// Converts a request with filled defaults into the VM's transaction.
///
/// Rejects fields incompatible with the transaction type. The VM checks
/// values such as the nonce and fees.
pub(crate) fn tx_env(request: &TransactionRequest, caller: Address) -> Result<TxEnv, Error> {
    let tx_type = request.transaction_type.unwrap_or_default();
    let tx_type = TxType::try_from(tx_type)
        .map_err(|_| Error::InvalidInput(format!("unsupported transaction type {tx_type:#x}")))?;
    check_tx_fields(request, tx_type)?;
    let input = request
        .input
        .clone()
        .try_into_unique_input()
        .map_err(|error| Error::InvalidInput(error.to_string()))?
        .unwrap_or_default();
    let is_dynamic_fee = !matches!(tx_type, TxType::Legacy | TxType::Eip2930);
    Ok(TxEnv {
        tx_type: tx_type as u8,
        caller,
        gas_limit: request.gas.unwrap_or_default(),
        gas_price: request
            .gas_price
            .or(request.max_fee_per_gas)
            .unwrap_or_default(),
        gas_priority_fee: is_dynamic_fee
            .then(|| request.max_priority_fee_per_gas.unwrap_or_default()),
        kind: request.to.unwrap_or(TxKind::Create),
        value: request.value.unwrap_or_default(),
        data: input,
        nonce: request.nonce.unwrap_or_default(),
        chain_id: request.chain_id,
        access_list: request.access_list.clone().unwrap_or_default(),
        blob_hashes: request.blob_versioned_hashes.clone().unwrap_or_default(),
        max_fee_per_blob_gas: request.max_fee_per_blob_gas.unwrap_or_default(),
        authorization_list: request
            .authorization_list
            .iter()
            .flatten()
            .cloned()
            .map(Either::Left)
            .collect(),
    })
}

/// Checks field compatibility and whether the type permits contract creation.
fn check_tx_fields(request: &TransactionRequest, tx_type: TxType) -> Result<(), Error> {
    let has_legacy_fee = matches!(tx_type, TxType::Legacy | TxType::Eip2930);
    let is_blob = tx_type == TxType::Eip4844;
    let fields = [
        ("gasPrice", request.gas_price.is_some(), has_legacy_fee),
        (
            "maxFeePerGas",
            request.max_fee_per_gas.is_some(),
            !has_legacy_fee,
        ),
        (
            "maxPriorityFeePerGas",
            request.max_priority_fee_per_gas.is_some(),
            !has_legacy_fee,
        ),
        (
            "accessList",
            request.access_list.is_some(),
            tx_type != TxType::Legacy,
        ),
        (
            "blobVersionedHashes",
            request.blob_versioned_hashes.is_some(),
            is_blob,
        ),
        ("sidecar", request.sidecar.is_some(), is_blob),
        (
            "maxFeePerBlobGas",
            request.max_fee_per_blob_gas.is_some(),
            is_blob,
        ),
        (
            "authorizationList",
            request.authorization_list.is_some(),
            tx_type == TxType::Eip7702,
        ),
    ];
    if let Some((field, ..)) = fields.iter().find(|(_, set, allowed)| *set && !allowed) {
        return Err(Error::InvalidInput(format!(
            "{field} is not a field of transaction type {:#x}",
            tx_type as u8
        )));
    }
    let creates = !matches!(request.to, Some(TxKind::Call(_)));
    if creates && matches!(tx_type, TxType::Eip4844 | TxType::Eip7702) {
        return Err(Error::InvalidInput(format!(
            "transaction type {:#x} cannot create a contract",
            tx_type as u8
        )));
    }
    Ok(())
}

/// Like geth's `eth_call`, a transaction that offers no fee is not held to
/// the base fee. A transaction that offers one is checked as sent.
pub(crate) fn skips_fee_checks(tx: &TxEnv) -> bool {
    tx.gas_price == 0 && tx.gas_priority_fee.unwrap_or_default() == 0
}
