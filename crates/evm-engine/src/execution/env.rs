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

use crate::{AccessListItem, EvmEngineError, EvmTransaction, EvmTransactionVariant};

use crate::ResolvedBlock;

pub(super) fn create_cfg_env(chain_id: u64, spec_id: SpecId) -> CfgEnv {
    CfgEnv::new_with_spec(spec_id).with_chain_id(chain_id)
}

pub(super) fn create_block_env(
    resolved_block: &ResolvedBlock,
    spec_id: SpecId,
) -> Result<BlockEnv, EvmEngineError> {
    let header = resolved_block.header();

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
        number: U256::from(resolved_block.number()),
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

pub(super) fn create_tx_env(transaction: &EvmTransaction) -> Result<TxEnv, EvmEngineError> {
    match &transaction.variant {
        EvmTransactionVariant::Legacy { gas_price } => {
            Ok(create_legacy_tx_env(transaction, *gas_price))
        }
        EvmTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => Ok(create_access_list_tx_env(
            transaction,
            *gas_price,
            access_list,
        )),
        EvmTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => Ok(create_dynamic_fee_tx_env(
            transaction,
            *max_fee_per_gas,
            *max_priority_fee_per_gas,
            access_list,
        )),
    }
}

fn create_legacy_tx_env(transaction: &EvmTransaction, gas_price: u128) -> TxEnv {
    base_tx_env(
        transaction,
        TransactionType::Legacy,
        MaterializedTxEnvValues {
            effective_gas_price: gas_price,
            gas_priority_fee: None,
            nonce: transaction.nonce,
            chain_id: transaction.chain_id,
            access_list: RevmAccessList::default(),
        },
    )
}

fn create_access_list_tx_env(
    transaction: &EvmTransaction,
    gas_price: u128,
    access_list: &[AccessListItem],
) -> TxEnv {
    base_tx_env(
        transaction,
        TransactionType::Eip2930,
        MaterializedTxEnvValues {
            effective_gas_price: gas_price,
            gas_priority_fee: None,
            nonce: transaction.nonce,
            chain_id: transaction.chain_id,
            access_list: map_access_list(access_list),
        },
    )
}

fn create_dynamic_fee_tx_env(
    transaction: &EvmTransaction,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    access_list: &[AccessListItem],
) -> TxEnv {
    base_tx_env(
        transaction,
        TransactionType::Eip1559,
        MaterializedTxEnvValues {
            effective_gas_price: max_fee_per_gas,
            gas_priority_fee: Some(max_priority_fee_per_gas),
            nonce: transaction.nonce,
            chain_id: transaction.chain_id,
            access_list: map_access_list(access_list),
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
    fn create_legacy_tx_env_uses_final_transaction_fields() {
        let transaction = EvmTransaction {
            chain_id: Chain::mainnet().id(),
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x11)),
            nonce: 7,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            variant: EvmTransactionVariant::Legacy { gas_price: 3 },
        };

        let tx_env = create_tx_env(&transaction).expect("tx env");

        assert_eq!(tx_env.gas_price, 3);
        assert_eq!(tx_env.nonce, 7);
        assert_eq!(tx_env.chain_id, Some(Chain::mainnet().id()));
    }

    #[test]
    fn create_dynamic_fee_tx_env_uses_final_transaction_fields() {
        let transaction = EvmTransaction {
            chain_id: Chain::mainnet().id(),
            from: Address::ZERO,
            to: Some(Address::repeat_byte(0x22)),
            nonce: 8,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            variant: EvmTransactionVariant::Eip1559 {
                max_fee_per_gas: 10,
                max_priority_fee_per_gas: 7,
                access_list: Vec::new(),
            },
        };

        let tx_env = create_tx_env(&transaction).expect("tx env");
        assert_eq!(tx_env.gas_price, 10);
        assert_eq!(tx_env.gas_priority_fee, Some(7));
    }
}
