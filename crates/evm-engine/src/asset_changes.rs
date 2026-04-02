use std::collections::HashMap;

use alloy_primitives::{Address, B256, U256, keccak256};

use crate::{
    AssetChange, AssetChangeAsset, AssetChangeType, AssetType,
    artifacts::{ExecutionArtifacts, RawExecutionLog},
    frames::{ExecutionFrame, ExecutionFrameStatus, ExecutionFrameType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Erc20Metadata {
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

pub(crate) fn fill_erc20_metadata<F>(asset_changes: &mut [AssetChange], mut load_metadata: F)
where
    F: FnMut(Address) -> Erc20Metadata,
{
    let mut metadata_by_token = HashMap::<Address, Erc20Metadata>::new();
    for asset_change in asset_changes {
        if !matches!(asset_change.asset_type, AssetType::Erc20) {
            continue;
        }

        let Some(asset) = asset_change.asset.as_mut() else {
            continue;
        };

        let metadata = metadata_by_token
            .entry(asset.token_address)
            .or_insert_with(|| load_metadata(asset.token_address));

        asset.symbol = metadata.symbol.clone();
        asset.decimals = metadata.decimals;
    }
}

fn erc20_transfer_topic0() -> B256 {
    keccak256("Transfer(address,address,uint256)".as_bytes())
}

fn is_zero_padded_address_topic(topic: &B256) -> bool {
    topic.as_slice()[..12].iter().all(|&byte| byte == 0)
}

pub(crate) fn extract_asset_changes(artifacts: &ExecutionArtifacts) -> Vec<AssetChange> {
    if !matches!(artifacts.status, crate::EvmExecutionStatus::Success) {
        return Vec::new();
    }

    let mut asset_changes = extract_native_asset_changes_from_frames(&artifacts.frames);

    for log in &artifacts.logs {
        if let Some(asset_change) = extract_erc20_transfer_from_log(log) {
            asset_changes.push(asset_change);
        }
    }

    asset_changes
}

fn extract_native_asset_changes_from_frames(frames: &[ExecutionFrame]) -> Vec<AssetChange> {
    let mut sorted_frames = frames.iter().collect::<Vec<_>>();
    sorted_frames.sort_by(|left, right| left.trace_address.cmp(&right.trace_address));

    let mut committed_stack = Vec::<bool>::new();
    let mut asset_changes = Vec::new();

    for frame in sorted_frames {
        committed_stack.truncate(frame.trace_address.len());

        let parent_committed = committed_stack.last().copied().unwrap_or(true);
        let frame_committed =
            parent_committed && matches!(frame.status, ExecutionFrameStatus::Success);

        if frame_committed {
            if let Some(asset_change) = extract_native_transfer_from_frame(frame) {
                asset_changes.push(asset_change);
            }
        }

        committed_stack.push(frame_committed);
    }

    asset_changes
}

fn extract_native_transfer_from_frame(frame: &ExecutionFrame) -> Option<AssetChange> {
    if frame.value.is_zero() {
        return None;
    }

    if !matches!(
        frame.frame_type,
        ExecutionFrameType::Call | ExecutionFrameType::Create | ExecutionFrameType::Create2
    ) {
        return None;
    }

    let to = frame.to?;

    Some(AssetChange {
        asset_type: AssetType::Native,
        change_type: AssetChangeType::Transfer,
        from: frame.from,
        to,
        amount: frame.value,
        asset: None,
    })
}
fn extract_erc20_transfer_from_log(log: &RawExecutionLog) -> Option<AssetChange> {
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
        AssetChange, AssetChangeAsset, AssetChangeType, AssetType, EvmExecutionStatus,
        SimulatedBlock,
        artifacts::{ExecutionArtifacts, RawExecutionLog},
        frames::{ExecutionFrame, ExecutionFrameStatus, ExecutionFrameType},
    };

    use super::{Erc20Metadata, extract_asset_changes, fill_erc20_metadata};

    fn sample_frame(
        frame_type: ExecutionFrameType,
        status: ExecutionFrameStatus,
        from: Address,
        to: Option<Address>,
        value: U256,
        trace_address: Vec<u64>,
    ) -> ExecutionFrame {
        ExecutionFrame {
            frame_type,
            status,
            from,
            to,
            code_address: None,
            value,
            input: Bytes::new(),
            output: Bytes::new(),
            gas: 21_000,
            gas_used: 21_000,
            trace_address,
        }
    }

    fn erc20_transfer_topic0() -> B256 {
        keccak256("Transfer(address,address,uint256)".as_bytes())
    }

    fn sample_execution_artifacts(
        status: EvmExecutionStatus,
        logs: Vec<RawExecutionLog>,
        frames: Vec<ExecutionFrame>,
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
            logs,
            frames,
        }
    }

    fn erc20_transfer_log(from: Address, to: Address, amount: U256) -> RawExecutionLog {
        RawExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word(), to.into_word()],
            data: Bytes::copy_from_slice(&amount.to_be_bytes::<32>()),
        }
    }

    #[test]
    fn extracts_single_native_transfer_for_success_with_value() {
        let from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("from address");
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let frames = vec![sample_frame(
            ExecutionFrameType::Call,
            ExecutionFrameStatus::Success,
            from,
            Some(to),
            U256::from(0x1234_u64),
            Vec::new(),
        )];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert_eq!(asset_changes.len(), 1);
        assert_eq!(asset_changes[0].asset_type, AssetType::Native);
        assert_eq!(asset_changes[0].change_type, AssetChangeType::Transfer);
        assert_eq!(asset_changes[0].from, from);
        assert_eq!(asset_changes[0].to, to);
        assert_eq!(asset_changes[0].amount, U256::from(0x1234_u64));
        assert_eq!(asset_changes[0].asset, None);
    }

    #[test]
    fn returns_empty_for_failed_execution() {
        let frames = vec![sample_frame(
            ExecutionFrameType::Call,
            ExecutionFrameStatus::Success,
            Address::from_str("0x1111111111111111111111111111111111111111").expect("from"),
            Some(Address::from_str("0x2222222222222222222222222222222222222222").expect("to")),
            U256::from(1_u64),
            Vec::new(),
        )];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Failed,
            Vec::new(),
            frames,
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn returns_empty_for_zero_value() {
        let from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("from address");
        let to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("to address");
        let frames = vec![sample_frame(
            ExecutionFrameType::Call,
            ExecutionFrameStatus::Success,
            from,
            Some(to),
            U256::ZERO,
            Vec::new(),
        )];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn extracts_native_transfer_for_contract_creation_with_value() {
        let from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("from address");
        let created = Address::from_str("0x5555555555555555555555555555555555555555")
            .expect("created contract");
        let frames = vec![sample_frame(
            ExecutionFrameType::Create,
            ExecutionFrameStatus::Success,
            from,
            Some(created),
            U256::from(1_u64),
            Vec::new(),
        )];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert_eq!(asset_changes.len(), 1);
        assert_eq!(asset_changes[0].asset_type, AssetType::Native);
        assert_eq!(asset_changes[0].from, from);
        assert_eq!(asset_changes[0].to, created);
        assert_eq!(asset_changes[0].amount, U256::from(1_u64));
    }

    #[test]
    fn extracts_erc20_transfer_from_standard_log() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");
        let log = erc20_transfer_log(from, to, U256::from(0x99_u64));

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![log],
            Vec::new(),
        ));

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

        let log = RawExecutionLog {
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

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![log],
            Vec::new(),
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn ignores_log_when_topics_len_is_not_three() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");

        let log = RawExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word()],
            data: Bytes::copy_from_slice(&U256::from(1_u64).to_be_bytes::<32>()),
        };

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![log],
            Vec::new(),
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn ignores_log_when_data_is_not_single_word() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let log = RawExecutionLog {
            log_index: 0,
            address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                .expect("token address"),
            topics: vec![erc20_transfer_topic0(), from.into_word(), to.into_word()],
            data: Bytes::from_static(&[0x12, 0x34]),
        };

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![log],
            Vec::new(),
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn ignores_log_when_address_topics_are_not_zero_padded() {
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let mut log = erc20_transfer_log(from, to, U256::from(0x99_u64));
        log.topics[1].as_mut_slice()[0] = 0x01;

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            vec![log],
            Vec::new(),
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn extracts_internal_native_transfers_from_committed_frames() {
        let root_from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("root from");
        let root_to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("root to");
        let inner_to =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("inner to");
        let created =
            Address::from_str("0x4444444444444444444444444444444444444444").expect("created");
        let frames = vec![
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                root_from,
                Some(root_to),
                U256::from(1_u64),
                Vec::new(),
            ),
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                root_to,
                Some(inner_to),
                U256::from(2_u64),
                vec![0],
            ),
            sample_frame(
                ExecutionFrameType::Create2,
                ExecutionFrameStatus::Success,
                inner_to,
                Some(created),
                U256::from(3_u64),
                vec![0, 0],
            ),
        ];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert_eq!(asset_changes.len(), 3);
        assert_eq!(asset_changes[0].from, root_from);
        assert_eq!(asset_changes[0].to, root_to);
        assert_eq!(asset_changes[0].amount, U256::from(1_u64));
        assert_eq!(asset_changes[1].from, root_to);
        assert_eq!(asset_changes[1].to, inner_to);
        assert_eq!(asset_changes[1].amount, U256::from(2_u64));
        assert_eq!(asset_changes[2].from, inner_to);
        assert_eq!(asset_changes[2].to, created);
        assert_eq!(asset_changes[2].amount, U256::from(3_u64));
    }

    #[test]
    fn ignores_reverted_frame_descendants_even_if_they_succeed() {
        let root_from =
            Address::from_str("0x1111111111111111111111111111111111111111").expect("root from");
        let root_to =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("root to");
        let reverted_to =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("reverted to");
        let reverted_child_to =
            Address::from_str("0x4444444444444444444444444444444444444444").expect("child to");
        let committed_sibling_to =
            Address::from_str("0x5555555555555555555555555555555555555555").expect("sibling to");
        let frames = vec![
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                root_from,
                Some(root_to),
                U256::ZERO,
                Vec::new(),
            ),
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Revert,
                root_to,
                Some(reverted_to),
                U256::from(2_u64),
                vec![0],
            ),
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                reverted_to,
                Some(reverted_child_to),
                U256::from(3_u64),
                vec![0, 0],
            ),
            sample_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                root_to,
                Some(committed_sibling_to),
                U256::from(4_u64),
                vec![1],
            ),
        ];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert_eq!(asset_changes.len(), 1);
        assert_eq!(asset_changes[0].from, root_to);
        assert_eq!(asset_changes[0].to, committed_sibling_to);
        assert_eq!(asset_changes[0].amount, U256::from(4_u64));
    }

    #[test]
    fn ignores_delegatecall_staticcall_and_callcode_for_native_changes() {
        let root = Address::from_str("0x1111111111111111111111111111111111111111").expect("root");
        let other = Address::from_str("0x2222222222222222222222222222222222222222").expect("other");
        let frames = vec![
            sample_frame(
                ExecutionFrameType::DelegateCall,
                ExecutionFrameStatus::Success,
                root,
                Some(other),
                U256::from(1_u64),
                vec![0],
            ),
            sample_frame(
                ExecutionFrameType::StaticCall,
                ExecutionFrameStatus::Success,
                root,
                Some(other),
                U256::from(2_u64),
                vec![1],
            ),
            sample_frame(
                ExecutionFrameType::CallCode,
                ExecutionFrameStatus::Success,
                root,
                Some(root),
                U256::from(3_u64),
                vec![2],
            ),
        ];

        let asset_changes = extract_asset_changes(&sample_execution_artifacts(
            EvmExecutionStatus::Success,
            Vec::new(),
            frames,
        ));

        assert!(asset_changes.is_empty());
    }

    #[test]
    fn fills_erc20_metadata_once_per_token() {
        let token = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("token");
        let from = Address::from_str("0x3333333333333333333333333333333333333333").expect("from");
        let to = Address::from_str("0x4444444444444444444444444444444444444444").expect("to");

        let mut asset_changes = vec![
            AssetChange {
                asset_type: AssetType::Native,
                change_type: AssetChangeType::Transfer,
                from,
                to,
                amount: U256::from(1_u64),
                asset: None,
            },
            AssetChange {
                asset_type: AssetType::Erc20,
                change_type: AssetChangeType::Transfer,
                from,
                to,
                amount: U256::from(2_u64),
                asset: Some(AssetChangeAsset {
                    token_address: token,
                    symbol: None,
                    decimals: None,
                }),
            },
            AssetChange {
                asset_type: AssetType::Erc20,
                change_type: AssetChangeType::Transfer,
                from: to,
                to: from,
                amount: U256::from(3_u64),
                asset: Some(AssetChangeAsset {
                    token_address: token,
                    symbol: None,
                    decimals: None,
                }),
            },
        ];

        let mut load_count = 0;

        fill_erc20_metadata(&mut asset_changes, |address| {
            assert_eq!(address, token);
            load_count += 1;

            Erc20Metadata {
                symbol: Some("USDC".to_string()),
                decimals: Some(6),
            }
        });

        assert_eq!(load_count, 1);
        assert_eq!(asset_changes[0].asset, None);

        let first_erc20 = asset_changes[1].asset.as_ref().expect("first erc20 asset");
        assert_eq!(first_erc20.symbol.as_deref(), Some("USDC"));
        assert_eq!(first_erc20.decimals, Some(6));

        let second_erc20 = asset_changes[2].asset.as_ref().expect("second erc20 asset");
        assert_eq!(second_erc20.symbol.as_deref(), Some("USDC"));
        assert_eq!(second_erc20.decimals, Some(6));
    }
}
