use crate::{AssetChange, EvmExecutionStatus, EvmTransaction};

pub(crate) fn extract_asset_changes(
    status: EvmExecutionStatus,
    transaction: &EvmTransaction,
) -> Vec<AssetChange> {
    if !matches!(status, EvmExecutionStatus::Success) {
        return Vec::new();
    }

    if transaction.value.is_zero() {
        return Vec::new();
    }

    let Some(to) = transaction.to else {
        return Vec::new();
    };

    vec![AssetChange {
        asset_type: crate::AssetType::Native,
        change_type: crate::AssetChangeType::Transfer,
        from: transaction.from,
        to,
        amount: transaction.value,
        asset: None,
    }]
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, Bytes, U256};

    use crate::{
        AccessListItem, AssetChangeType, AssetType, EvmExecutionStatus, EvmTransaction,
        EvmTransactionType,
    };

    use super::extract_asset_changes;

    fn sample_transaction(value: U256, to: Option<Address>) -> EvmTransaction {
        EvmTransaction {
            tx_type: EvmTransactionType::Legacy,
            chain_id: 1,
            from: Address::from_str("0x1111111111111111111111111111111111111111")
                .expect("from address"),
            to,
            nonce: 0,
            gas_limit: 21_000,
            value,
            data: Bytes::new(),
            access_list: Vec::<AccessListItem>::new(),
            gas_price: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }

    #[test]
    fn extracts_single_native_transfer_for_success_with_value() {
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let transaction = sample_transaction(U256::from(0x1234_u64), Some(to));

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction);

        assert_eq!(asset_changes.len(), 1);
        assert_eq!(asset_changes[0].asset_type, AssetType::Native);
        assert_eq!(asset_changes[0].change_type, AssetChangeType::Transfer);
        assert_eq!(asset_changes[0].from, transaction.from);
        assert_eq!(asset_changes[0].to, to);
        assert_eq!(asset_changes[0].amount, U256::from(0x1234_u64));
        assert_eq!(asset_changes[0].asset, None);
    }

    #[test]
    fn returns_empty_for_failed_execution() {
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let transaction = sample_transaction(U256::from(1_u64), Some(to));

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Failed, &transaction);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn returns_empty_for_zero_value() {
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let transaction = sample_transaction(U256::ZERO, Some(to));

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn returns_empty_for_contract_creation_with_value() {
        let transaction = sample_transaction(U256::from(1_u64), None);

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction);

        assert!(asset_changes.is_empty());
    }
}
