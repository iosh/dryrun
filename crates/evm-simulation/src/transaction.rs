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
/// - `gas`: the block gas limit, capped by the per-transaction limit.
/// - `nonce`: the sender's nonce in the state.
/// - `chainId`: the chain's id.
/// - fees: zero; see [`skips_fee_checks`].
/// - blob fee: the block's blob gas price.
pub(crate) fn fill_defaults(
    request: &mut TransactionRequest,
    env: &EvmEnv,
    state_nonce: impl FnOnce() -> Result<u64, Error>,
) -> Result<(), Error> {
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
    if request.has_eip4844_blob_data() && request.max_fee_per_blob_gas.is_none() {
        request.max_fee_per_blob_gas = env.block_env.blob_gasprice();
    }
    Ok(())
}

/// Converts a request with filled defaults into the VM's transaction.
/// Validity rules are left to the VM.
pub(crate) fn tx_env(request: &TransactionRequest, caller: Address) -> Result<TxEnv, Error> {
    let input = request
        .input
        .clone()
        .try_into_unique_input()
        .map_err(|error| Error::InvalidInput(error.to_string()))?
        .unwrap_or_default();
    if request.gas_price.is_some()
        && (request.max_fee_per_gas.is_some() || request.max_priority_fee_per_gas.is_some())
    {
        return Err(Error::InvalidInput(
            "gasPrice cannot be combined with maxFeePerGas or maxPriorityFeePerGas".into(),
        ));
    }

    let tx_type = match request.transaction_type {
        Some(tx_type) => tx_type,
        None => request.minimal_tx_type() as u8,
    };
    let is_dynamic_fee = tx_type >= TxType::Eip1559 as u8;
    Ok(TxEnv {
        tx_type,
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

/// Like geth's `eth_call`, a transaction that offers no fee is not held to
/// the base fee. A transaction that offers one is checked as sent.
pub(crate) fn skips_fee_checks(tx: &TxEnv) -> bool {
    tx.gas_price == 0 && tx.gas_priority_fee.unwrap_or_default() == 0
}
