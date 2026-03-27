use alloy_primitives::{Address, B256, U256, keccak256};

use crate::{
    AssetChange, AssetChangeAsset, AssetChangeType, AssetType, EvmExecutionLog, EvmExecutionStatus,
    EvmTransaction,
};

fn erc20_transfer_topic0() -> B256 {
    keccak256("Transfer(address,address,uint256)".as_bytes())
}

fn is_zero_padded_address_topic(topic: &B256) -> bool {
    topic.as_slice()[..12].iter().all(|&byte| byte == 0)
}

pub(crate) fn extract_asset_changes(
    status: EvmExecutionStatus,
    transaction: &EvmTransaction,
    logs: &[EvmExecutionLog],
) -> Vec<AssetChange> {
    if !matches!(status, EvmExecutionStatus::Success) {
        return Vec::new();
    }

    let mut asset_changes = Vec::new();

    if let Some(native_change) = extract_native_top_level_change(transaction) {
        asset_changes.push(native_change);
    }

    for log in logs {
        if let Some(asset_change) = extract_erc20_transfer_from_log(log) {
            asset_changes.push(asset_change);
        }
    }

    asset_changes
}

fn extract_native_top_level_change(transaction: &EvmTransaction) -> Option<AssetChange> {
    if transaction.value.is_zero() {
        return None;
    }

    let Some(to) = transaction.to else {
        return None;
    };

    Some(AssetChange {
        asset_type: AssetType::Native,
        change_type: AssetChangeType::Transfer,
        from: transaction.from,
        to,
        amount: transaction.value,
        asset: None,
    })
}

fn extract_erc20_transfer_from_log(log: &EvmExecutionLog) -> Option<AssetChange> {
    if log.topics.len() != 3 {
        return None;
    }

    if log.topics[0] != erc20_transfer_topic0() {
        return None;
    }

    if log.data.len() != 32 {
        return None;
    }

    if !is_zero_padded_address_topic(&log.topics[1]) {
        return None;
    }

    if !is_zero_padded_address_topic(&log.topics[2]) {
        return None;
    }

    let from = Address::from_word(log.topics[1]);
    let to = Address::from_word(log.topics[2]);
    let amount = U256::from_be_slice(log.data.as_ref());

    Some(AssetChange {
        asset_type: AssetType::Erc20,
        change_type: AssetChangeType::Transfer,
        from,
        to,
        amount,
        asset: Some(AssetChangeAsset {
            token_address: log.address,
            symbol: None,
            decimals: None,
        }),
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256, keccak256};

    use crate::{
        AccessListItem, AssetChangeType, AssetType, EvmExecutionLog, EvmExecutionStatus,
        EvmTransaction, EvmTransactionType,
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

    fn erc20_transfer_topic0() -> B256 {
        keccak256("Transfer(address,address,uint256)".as_bytes())
    }

    fn erc20_transfer_log(from: Address, to: Address, amount: U256) -> EvmExecutionLog {
        EvmExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word(), to.into_word()],
            data: Bytes::copy_from_slice(&amount.to_be_bytes::<32>()),
        }
    }

    #[test]
    fn extracts_single_native_transfer_for_success_with_value() {
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let transaction = sample_transaction(U256::from(0x1234_u64), Some(to));

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[]);

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

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Failed, &transaction, &[]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn returns_empty_for_zero_value() {
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let transaction = sample_transaction(U256::ZERO, Some(to));

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn returns_empty_for_contract_creation_with_value() {
        let transaction = sample_transaction(U256::from(1_u64), None);

        let asset_changes = extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn extracts_erc20_transfer_from_standard_log() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");
        let log = erc20_transfer_log(from, to, U256::from(0x99_u64));
        let transaction = sample_transaction(U256::ZERO, Some(to));

        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert_eq!(asset_changes.len(), 1);
        assert_eq!(asset_changes[0].asset_type, AssetType::Erc20);
        assert_eq!(asset_changes[0].change_type, AssetChangeType::Transfer);
        assert_eq!(asset_changes[0].from, from);
        assert_eq!(asset_changes[0].to, to);
        assert_eq!(asset_changes[0].amount, U256::from(0x99_u64));
        assert_eq!(
            asset_changes[0]
                .asset
                .as_ref()
                .expect("erc20 asset")
                .token_address,
            Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("token")
        );
    }

    #[test]
    fn ignores_log_when_topic0_does_not_match() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let log = EvmExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![
                keccak256("Approval(address,address,uint256)".as_bytes()),
                from.into_word(),
                to.into_word(),
            ],
            data: Bytes::copy_from_slice(&U256::from(1_u64).to_be_bytes::<32>()),
        };

        let transaction = sample_transaction(U256::ZERO, Some(to));
        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn ignores_log_when_topics_len_is_not_three() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let log = EvmExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word()],
            data: Bytes::copy_from_slice(&U256::from(1_u64).to_be_bytes::<32>()),
        };

        let transaction = sample_transaction(U256::ZERO, Some(to));
        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn ignores_log_when_data_is_not_single_word() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let log = EvmExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word(), to.into_word()],
            data: Bytes::from_static(&[0x12, 0x34]),
        };

        let transaction = sample_transaction(U256::ZERO, Some(to));
        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn keeps_native_and_erc20_changes_together() {
        let native_to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("native to");
        let erc20_from =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("erc20 from");
        let erc20_to =
            Address::from_str("0x4444444444444444444444444444444444444444").expect("erc20 to");

        let transaction = sample_transaction(U256::from(0x1234_u64), Some(native_to));
        let log = erc20_transfer_log(erc20_from, erc20_to, U256::from(0x99_u64));

        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert_eq!(asset_changes.len(), 2);
        assert_eq!(asset_changes[0].asset_type, AssetType::Native);
        assert_eq!(asset_changes[1].asset_type, AssetType::Erc20);
    }

    #[test]
    fn ignores_log_when_address_topics_are_not_zero_padded() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let mut log = erc20_transfer_log(from, to, U256::from(0x99_u64));
        log.topics[1].as_mut_slice()[0] = 0x01;

        let transaction = sample_transaction(U256::ZERO, Some(to));
        let asset_changes =
            extract_asset_changes(EvmExecutionStatus::Success, &transaction, &[log]);

        assert!(asset_changes.is_empty());
    }
}
