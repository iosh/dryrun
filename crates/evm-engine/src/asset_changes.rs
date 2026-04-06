use std::collections::HashMap;

use alloy_primitives::Address;

use crate::{
    Asset, Change, TransferChange,
    artifacts::ExecutionArtifacts,
    change_observer::ObservedChange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Erc20Metadata {
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

pub(crate) fn fill_erc20_metadata<F>(changes: &mut [Change], mut load_metadata: F)
where
    F: FnMut(Address) -> Erc20Metadata,
{
    let mut metadata_by_token = HashMap::<Address, Erc20Metadata>::new();

    for change in changes {
        let Some(asset) = asset_mut(change) else {
            continue;
        };

        let Asset::Erc20 {
            contract_address,
            symbol,
            decimals,
            ..
        } = asset
        else {
            continue;
        };

        let token_address = *contract_address;
        let metadata = metadata_by_token
            .entry(token_address)
            .or_insert_with(|| load_metadata(token_address));

        *symbol = metadata.symbol.clone();
        *decimals = metadata.decimals;
    }
}

pub(crate) fn extract_asset_changes(artifacts: &ExecutionArtifacts) -> Vec<Change> {
    if !matches!(artifacts.status, crate::EvmExecutionStatus::Success) {
        return Vec::new();
    }

    artifacts
        .observed_changes
        .iter()
        .cloned()
        .map(observed_change_to_change)
        .collect()
}

fn asset_mut(change: &mut Change) -> Option<&mut Asset> {
    match change {
        Change::Transfer(change) => Some(&mut change.asset),
        Change::Mint(change) => Some(&mut change.asset),
        Change::Burn(change) => Some(&mut change.asset),
        Change::Approval(change) => Some(&mut change.asset),
        Change::ApprovalForAll(_) => None,
    }
}

fn observed_change_to_change(change: ObservedChange) -> Change {
    match change {
        ObservedChange::NativeTransfer { from, to, amount } => Change::Transfer(TransferChange {
            asset: Asset::Native {
                symbol: None,
                decimals: None,
            },
            from,
            to,
            amount: Some(amount),
        }),
        ObservedChange::Erc20Transfer {
            contract_address,
            from,
            to,
            amount,
        } => Change::Transfer(TransferChange {
            asset: Asset::Erc20 {
                contract_address,
                symbol: None,
                decimals: None,
                name: None,
            },
            from,
            to,
            amount: Some(amount),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};

    use crate::{
        Asset, Change, EvmExecutionStatus, SimulatedBlock, TransferChange,
        artifacts::ExecutionArtifacts,
        change_observer::ObservedChange,
    };

    use super::{Erc20Metadata, extract_asset_changes, fill_erc20_metadata};

    fn sample_execution_artifacts(
        status: EvmExecutionStatus,
        observed_changes: Vec<ObservedChange>,
    ) -> ExecutionArtifacts {
        ExecutionArtifacts {
            chain_id: 1,
            block: SimulatedBlock {
                number: 1,
                hash: B256::ZERO,
            },
            status,
            gas_used: 21_000,
            gas_limit: 50_000,
            output: Bytes::new(),
            failure: None,
            observed_changes,
            logs: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn transfer(change: &Change) -> &TransferChange {
        match change {
            Change::Transfer(change) => change,
            other => panic!("expected transfer change, got {other:?}"),
        }
    }

    #[test]
    fn returns_empty_for_failed_execution() {
        let changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Failed,
            vec![ObservedChange::NativeTransfer {
                from: Address::from_str("0x1111111111111111111111111111111111111111").unwrap(),
                to: Address::from_str("0x2222222222222222222222222222222222222222").unwrap(),
                amount: U256::from(1_u64),
            }],
        ));

        assert!(changes.is_empty());
    }

    #[test]
    fn maps_observed_changes_in_order() {
        let native_from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("from");
        let native_to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to");
        let token =
            Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("token");
        let token_from =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let token_to =
            Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![
                ObservedChange::NativeTransfer {
                    from: native_from,
                    to: native_to,
                    amount: U256::from(1_u64),
                },
                ObservedChange::Erc20Transfer {
                    contract_address: token,
                    from: token_from,
                    to: token_to,
                    amount: U256::from(2_u64),
                },
            ],
        ));

        assert_eq!(changes.len(), 2);

        let first = transfer(&changes[0]);
        assert!(matches!(first.asset, Asset::Native { .. }));
        assert_eq!(first.from, native_from);
        assert_eq!(first.to, native_to);
        assert_eq!(first.amount, Some(U256::from(1_u64)));

        let second = transfer(&changes[1]);
        assert!(matches!(
            second.asset,
            Asset::Erc20 {
                contract_address,
                symbol: None,
                decimals: None,
                name: None,
            } if contract_address == token
        ));
        assert_eq!(second.from, token_from);
        assert_eq!(second.to, token_to);
        assert_eq!(second.amount, Some(U256::from(2_u64)));
    }

    #[test]
    fn fills_erc20_metadata_once_per_token() {
        let token = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("token");
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let mut changes = vec![
            Change::Transfer(TransferChange {
                asset: Asset::Native {
                    symbol: None,
                    decimals: None,
                },
                from,
                to,
                amount: Some(U256::from(1_u64)),
            }),
            Change::Transfer(TransferChange {
                asset: Asset::Erc20 {
                    contract_address: token,
                    symbol: None,
                    decimals: None,
                    name: None,
                },
                from,
                to,
                amount: Some(U256::from(2_u64)),
            }),
            Change::Transfer(TransferChange {
                asset: Asset::Erc20 {
                    contract_address: token,
                    symbol: None,
                    decimals: None,
                    name: None,
                },
                from: to,
                to: from,
                amount: Some(U256::from(3_u64)),
            }),
        ];

        let mut load_count = 0;

        fill_erc20_metadata(&mut changes, |address| {
            assert_eq!(address, token);
            load_count += 1;

            Erc20Metadata {
                symbol: Some("USDC".to_string()),
                decimals: Some(6),
            }
        });

        assert_eq!(load_count, 1);
        assert!(matches!(
            transfer(&changes[0]).asset,
            Asset::Native {
                symbol: None,
                decimals: None,
            }
        ));

        let first_erc20 = transfer(&changes[1]);
        assert!(matches!(
            first_erc20.asset,
            Asset::Erc20 {
                contract_address,
                symbol: Some(ref symbol),
                decimals: Some(6),
                name: None,
            } if contract_address == token && symbol == "USDC"
        ));

        let second_erc20 = transfer(&changes[2]);
        assert!(matches!(
            second_erc20.asset,
            Asset::Erc20 {
                contract_address,
                symbol: Some(ref symbol),
                decimals: Some(6),
                name: None,
            } if contract_address == token && symbol == "USDC"
        ));
    }
}
