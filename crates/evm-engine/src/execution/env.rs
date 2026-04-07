use alloy::consensus::BlockHeader;
use alloy_primitives::U256;
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        block::BlobExcessGasAndPrice,
        transaction::{
            AccessList as RevmAccessList, AccessListItem as RevmAccessListItem, TransactionType,
        },
    },
    primitives::{TxKind, hardfork::SpecId},
};

use crate::{AccessListItem, EvmEngineError, EvmTransaction, EvmTransactionType};

use super::provider::ResolvedExecutionBlock;

pub(super) fn validate_requested_chain_id(
    requested_chain_id: Option<u64>,
    actual_chain_id: u64,
) -> Result<(), EvmEngineError> {
    if let Some(requested_chain_id) = requested_chain_id
        && requested_chain_id != actual_chain_id
    {
        return Err(EvmEngineError::not_supported(format!(
            "transaction.chainId does not match the execution chain: requested={requested_chain_id}, actual={actual_chain_id}",
        )));
    }

    Ok(())
}

pub(super) fn create_cfg_env(
    transaction: &EvmTransaction,
    chain_id: u64,
    spec_id: SpecId,
) -> CfgEnv {
    let mut cfg = CfgEnv::new_with_spec(spec_id).with_chain_id(chain_id);
    configure_preview_cfg(&mut cfg, transaction);
    cfg
}

pub(super) fn create_block_env(
    resolved_block: &ResolvedExecutionBlock,
    spec_id: SpecId,
) -> Result<BlockEnv, EvmEngineError> {
    let header = &resolved_block.block.header;

    let basefee = if spec_id.is_enabled_in(SpecId::LONDON) {
        header.base_fee_per_gas().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing base fee for spec {spec_id:?}"
            ))
        })?
    } else {
        0
    };

    let prevrandao = if spec_id.is_enabled_in(SpecId::MERGE) {
        Some(header.mix_hash().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing prev randao for spec {spec_id:?}"
            ))
        })?)
    } else {
        None
    };

    let blob_excess_gas_and_price = if spec_id.is_enabled_in(SpecId::CANCUN) {
        let excess_blob_gas = header.excess_blob_gas().ok_or_else(|| {
            EvmEngineError::block_context_error(format!(
                "rpc block header is missing excess blob gas for spec {spec_id:?}"
            ))
        })?;

        Some(BlobExcessGasAndPrice::new_with_spec(
            excess_blob_gas,
            spec_id,
        ))
    } else {
        None
    };

    Ok(BlockEnv {
        number: U256::from(resolved_block.block.number()),
        beneficiary: header.beneficiary(),
        timestamp: U256::from(header.timestamp()),
        gas_limit: header.gas_limit(),
        basefee,
        difficulty: header.difficulty(),
        prevrandao,
        blob_excess_gas_and_price,
        slot_num: 0,
    })
}

pub(super) fn create_tx_env(
    transaction: &EvmTransaction,
    chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    match transaction.tx_type {
        EvmTransactionType::Legacy => create_legacy_tx_env(transaction, chain_id),
        EvmTransactionType::AccessList => Ok(create_access_list_tx_env(transaction, chain_id)),
        EvmTransactionType::DynamicFee => Ok(create_dynamic_fee_tx_env(transaction, chain_id)),
    }
}

fn configure_preview_cfg(cfg: &mut CfgEnv, transaction: &EvmTransaction) {
    cfg.disable_nonce_check = transaction.nonce.is_none();
    cfg.disable_balance_check = true;
    cfg.disable_eip3607 = true;
    cfg.disable_base_fee = !transaction_has_explicit_fee_constraints(transaction);
    cfg.disable_fee_charge = true;
}

fn transaction_has_explicit_fee_constraints(transaction: &EvmTransaction) -> bool {
    transaction.gas_price.is_some()
        || transaction.max_fee_per_gas.is_some()
        || transaction.max_priority_fee_per_gas.is_some()
}

fn create_legacy_tx_env(
    transaction: &EvmTransaction,
    chain_id: u64,
) -> Result<TxEnv, EvmEngineError> {
    if !transaction.access_list.is_empty() {
        return Err(EvmEngineError::internal(
            "legacy transaction must not include access_list",
        ));
    }

    Ok(base_tx_env(
        transaction,
        TransactionType::Legacy,
        MaterializedTxEnvValues {
            effective_gas_price: transaction.gas_price.unwrap_or(0),
            gas_priority_fee: None,
            nonce: materialize_preview_nonce(transaction),
            chain_id,
            access_list: RevmAccessList::default(),
        },
    ))
}

fn create_access_list_tx_env(transaction: &EvmTransaction, chain_id: u64) -> TxEnv {
    base_tx_env(
        transaction,
        TransactionType::Eip2930,
        MaterializedTxEnvValues {
            effective_gas_price: transaction.gas_price.unwrap_or(0),
            gas_priority_fee: None,
            nonce: materialize_preview_nonce(transaction),
            chain_id,
            access_list: map_access_list(&transaction.access_list),
        },
    )
}

fn create_dynamic_fee_tx_env(transaction: &EvmTransaction, chain_id: u64) -> TxEnv {
    let (materialized_max_fee_per_gas, materialized_max_priority_fee_per_gas) =
        materialize_preview_dynamic_fee(transaction);

    base_tx_env(
        transaction,
        TransactionType::Eip1559,
        MaterializedTxEnvValues {
            effective_gas_price: materialized_max_fee_per_gas,
            gas_priority_fee: Some(materialized_max_priority_fee_per_gas),
            nonce: materialize_preview_nonce(transaction),
            chain_id,
            access_list: map_access_list(&transaction.access_list),
        },
    )
}

struct MaterializedTxEnvValues {
    effective_gas_price: u128,
    gas_priority_fee: Option<u128>,
    nonce: u64,
    chain_id: u64,
    access_list: RevmAccessList,
}

fn materialize_preview_nonce(transaction: &EvmTransaction) -> u64 {
    transaction.nonce.unwrap_or(0)
}

fn base_tx_env(
    transaction: &EvmTransaction,
    tx_type: TransactionType,
    values: MaterializedTxEnvValues,
) -> TxEnv {
    TxEnv {
        tx_type: tx_type as u8,
        caller: transaction.from,
        gas_limit: transaction.gas_limit,
        gas_price: values.effective_gas_price,
        kind: TxKind::from(transaction.to),
        value: transaction.value,
        data: transaction.data.clone(),
        nonce: values.nonce,
        chain_id: Some(values.chain_id),
        access_list: values.access_list,
        gas_priority_fee: values.gas_priority_fee,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    }
}

fn materialize_preview_dynamic_fee(transaction: &EvmTransaction) -> (u128, u128) {
    match (
        transaction.max_fee_per_gas,
        transaction.max_priority_fee_per_gas,
    ) {
        (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) => {
            (max_fee_per_gas, max_priority_fee_per_gas)
        }
        (Some(max_fee_per_gas), None) => (max_fee_per_gas, 0),
        (None, Some(max_priority_fee_per_gas)) => {
            (max_priority_fee_per_gas, max_priority_fee_per_gas)
        }
        (None, None) => (0, 0),
    }
}

fn map_access_list(items: &[AccessListItem]) -> RevmAccessList {
    items
        .iter()
        .map(|item| RevmAccessListItem {
            address: item.address,
            storage_keys: item.storage_keys.clone(),
        })
        .collect::<Vec<_>>()
        .into()
}

#[cfg(test)]
mod tests {
    use alloy_chains::Chain;
    use alloy_primitives::{Address, Bytes, U256};

    use super::*;

    #[test]
    fn create_legacy_tx_env_defaults_call_like_send_fields() {
        let transaction = EvmTransaction {
            tx_type: EvmTransactionType::Legacy,
            requested_chain_id: None,
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x11)),
            nonce: None,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            access_list: Vec::new(),
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");

        assert_eq!(tx_env.gas_price, 0);
        assert_eq!(tx_env.nonce, 0);
        assert_eq!(tx_env.chain_id, Some(Chain::mainnet().id()));
    }

    #[test]
    fn create_dynamic_fee_tx_env_materializes_missing_fee_fields() {
        let mut transaction = EvmTransaction {
            tx_type: EvmTransactionType::DynamicFee,
            requested_chain_id: None,
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x22)),
            nonce: None,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            access_list: Vec::new(),
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        };

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");
        assert_eq!(tx_env.gas_price, 0);
        assert_eq!(tx_env.gas_priority_fee, Some(0));

        transaction.max_priority_fee_per_gas = Some(7);

        let tx_env = create_tx_env(&transaction, Chain::mainnet().id()).expect("tx env");
        assert_eq!(tx_env.gas_price, 7);
        assert_eq!(tx_env.gas_priority_fee, Some(7));
    }
}
