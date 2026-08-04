use alloy::consensus::{BlockHeader, Header};
use alloy::primitives::U256;
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
use simulation_transaction::{AccessListItem, Transaction, TransactionVariant};

use super::EvmExecutionError;

pub(super) fn create_cfg_env(chain_id: u64, spec_id: SpecId) -> CfgEnv {
    CfgEnv::new_with_spec(spec_id).with_chain_id(chain_id)
}

pub(super) fn create_block_env(
    header: &Header,
    spec_id: SpecId,
) -> Result<BlockEnv, EvmExecutionError> {
    let basefee = if spec_id.is_enabled_in(SpecId::LONDON) {
        header.base_fee_per_gas().ok_or_else(|| {
            EvmExecutionError::BlockContext(format!(
                "rpc block header is missing base fee for spec {spec_id:?}"
            ))
        })?
    } else {
        0
    };

    let prevrandao = if spec_id.is_enabled_in(SpecId::MERGE) {
        Some(header.mix_hash().ok_or_else(|| {
            EvmExecutionError::BlockContext(format!(
                "rpc block header is missing prev randao for spec {spec_id:?}"
            ))
        })?)
    } else {
        None
    };

    let blob_excess_gas_and_price = if spec_id.is_enabled_in(SpecId::CANCUN) {
        let excess_blob_gas = header.excess_blob_gas().ok_or_else(|| {
            EvmExecutionError::BlockContext(format!(
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
        number: U256::from(header.number()),
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

pub(super) fn create_tx_env(transaction: &Transaction) -> TxEnv {
    match &transaction.variant {
        TransactionVariant::Legacy { gas_price } => base_tx_env(
            transaction,
            TransactionType::Legacy,
            *gas_price,
            None,
            Default::default(),
        ),
        TransactionVariant::AccessList {
            gas_price,
            access_list,
        } => base_tx_env(
            transaction,
            TransactionType::Eip2930,
            *gas_price,
            None,
            map_access_list(access_list),
        ),
        TransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => base_tx_env(
            transaction,
            TransactionType::Eip1559,
            *max_fee_per_gas,
            Some(*max_priority_fee_per_gas),
            map_access_list(access_list),
        ),
    }
}

fn base_tx_env(
    transaction: &Transaction,
    tx_type: TransactionType,
    gas_price: u128,
    gas_priority_fee: Option<u128>,
    access_list: RevmAccessList,
) -> TxEnv {
    TxEnv {
        tx_type: tx_type as u8,
        caller: transaction.from,
        gas_limit: transaction.gas_limit,
        gas_price,
        kind: TxKind::from(transaction.to),
        value: transaction.value,
        data: transaction.data.clone(),
        nonce: transaction.nonce,
        chain_id: Some(transaction.chain_id),
        access_list,
        gas_priority_fee,
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
