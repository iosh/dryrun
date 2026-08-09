use crate::{
    AccessListItem, CompleteTransaction, CompleteTransactionVariant, EthereumExecutionSpec,
    EvmBlockEnvironmentError,
};
use alloy::consensus::{BlockHeader, Header};
use alloy::primitives::{TxKind, U256};
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        block::BlobExcessGasAndPrice,
        transaction::{
            AccessList as RevmAccessList, AccessListItem as RevmAccessListItem, TransactionType,
        },
    },
    primitives::hardfork::SpecId,
};

pub(super) fn create_cfg_env(chain_id: u64, execution_spec: EthereumExecutionSpec) -> CfgEnv {
    let mut cfg = CfgEnv::new_with_spec(execution_spec.spec_id).with_chain_id(chain_id);
    if let Some(blob_params) = execution_spec.blob_params {
        cfg.set_max_blobs_per_tx(blob_params.max_blobs_per_tx);
    }
    cfg
}

pub(super) fn create_block_env(
    header: &Header,
    execution_spec: EthereumExecutionSpec,
) -> Result<BlockEnv, EvmBlockEnvironmentError> {
    let spec_id = execution_spec.spec_id;
    let basefee = if spec_id.is_enabled_in(SpecId::LONDON) {
        header
            .base_fee_per_gas()
            .ok_or(EvmBlockEnvironmentError::MissingBaseFee {
                block_number: header.number(),
            })?
    } else {
        0
    };

    let prevrandao = if spec_id.is_enabled_in(SpecId::MERGE) {
        Some(
            header
                .mix_hash()
                .ok_or(EvmBlockEnvironmentError::MissingPrevRandao {
                    block_number: header.number(),
                })?,
        )
    } else {
        None
    };

    let blob_excess_gas_and_price = if let Some(blob_params) = execution_spec.blob_params {
        let excess_blob_gas =
            header
                .excess_blob_gas()
                .ok_or(EvmBlockEnvironmentError::MissingExcessBlobGas {
                    block_number: header.number(),
                })?;

        Some(BlobExcessGasAndPrice {
            excess_blob_gas,
            blob_gasprice: blob_params.calc_blob_fee(excess_blob_gas),
        })
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

pub(super) fn create_tx_env(transaction: &CompleteTransaction) -> TxEnv {
    match &transaction.variant {
        CompleteTransactionVariant::Legacy { gas_price } => base_tx_env(
            transaction,
            TransactionType::Legacy,
            *gas_price,
            None,
            Default::default(),
        ),
        CompleteTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => base_tx_env(
            transaction,
            TransactionType::Eip2930,
            *gas_price,
            None,
            map_access_list(access_list),
        ),
        CompleteTransactionVariant::Eip1559 {
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
        CompleteTransactionVariant::Eip4844 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            max_fee_per_blob_gas,
            access_list,
            blob_versioned_hashes,
        } => {
            let mut tx = base_tx_env(
                transaction,
                TransactionType::Eip4844,
                *max_fee_per_gas,
                Some(*max_priority_fee_per_gas),
                map_access_list(access_list),
            );
            tx.blob_hashes = blob_versioned_hashes.clone();
            tx.max_fee_per_blob_gas = *max_fee_per_blob_gas;
            tx
        }
        CompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
            authorization_list,
        } => {
            let mut tx = base_tx_env(
                transaction,
                TransactionType::Eip7702,
                *max_fee_per_gas,
                Some(*max_priority_fee_per_gas),
                map_access_list(access_list),
            );
            tx.set_signed_authorization(authorization_list.clone());
            tx
        }
    }
}

fn base_tx_env(
    transaction: &CompleteTransaction,
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
        kind: transaction.to.map_or(TxKind::Create, TxKind::Call),
        value: transaction.value,
        data: transaction.input.clone(),
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
