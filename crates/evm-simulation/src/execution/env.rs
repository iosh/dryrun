use crate::transaction::checked_fee;
use crate::{AccessListItem, EthereumExecutionSpec, EvmBlockEnvironmentError, TypedTransaction};
use alloy::consensus::{BlockHeader, Header};
use alloy::primitives::{TxKind, U256};
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        block::BlobExcessGasAndPrice,
        transaction::{AccessList as RevmAccessList, AccessListItem as RevmAccessListItem},
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

pub(super) fn create_tx_env(
    transaction: &TypedTransaction,
) -> Result<TxEnv, crate::TransactionInputError> {
    let common = transaction.common();
    let dynamic = transaction.dynamic_fees();
    let gas_price = checked_fee(
        if dynamic.is_some() {
            "maxFeePerGas"
        } else {
            "gasPrice"
        },
        transaction.gas_price_cap(),
    )?;
    let gas_priority_fee = dynamic
        .map(|fees| checked_fee("maxPriorityFeePerGas", fees.max_priority_fee_per_gas))
        .transpose()?;
    let mut tx = TxEnv {
        tx_type: transaction.transaction_type() as u8,
        caller: common.from,
        gas_limit: common.gas_limit,
        gas_price,
        kind: common.to.map_or(TxKind::Create, TxKind::Call),
        value: common.value,
        data: common.input.clone(),
        nonce: common.nonce,
        chain_id: Some(common.chain_id),
        access_list: map_access_list(transaction.access_list()),
        gas_priority_fee,
        blob_hashes: Vec::new(),
        max_fee_per_blob_gas: 0,
        authorization_list: Vec::new(),
    };
    match transaction {
        crate::TypedTransaction::Eip4844 {
            max_fee_per_blob_gas,
            blob_versioned_hashes,
            ..
        } => {
            tx.max_fee_per_blob_gas = checked_fee("maxFeePerBlobGas", *max_fee_per_blob_gas)?;
            tx.blob_hashes = blob_versioned_hashes.clone();
        }
        crate::TypedTransaction::Eip7702 {
            authorization_list, ..
        } => tx.set_signed_authorization(authorization_list.clone()),
        _ => {}
    }
    Ok(tx)
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
