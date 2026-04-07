use std::{collections::HashMap, str::FromStr};

use alloy_primitives::{Address, B256, Bytes, U256};

use crate::{
    ApprovalChange, ApprovalForAllChange, Asset, Change, Collection, SimulatedBlock,
    change_observation::Observation, execution::ExecutionArtifacts,
};

use super::{
    ChangeDetectionPipeline, ContractKind, DetectionContext, DetectionOutcome, DetectionSupport,
    Erc20Metadata, ObservationDetector, approval_for_all_topic0, approval_topic0, transfer_topic0,
};

struct TestSupport {
    kinds: HashMap<Address, ContractKind>,
    metadata: HashMap<Address, Erc20Metadata>,
    contract_kind_loads: usize,
    erc20_metadata_loads: usize,
}

impl TestSupport {
    fn new() -> Self {
        Self {
            kinds: HashMap::new(),
            metadata: HashMap::new(),
            contract_kind_loads: 0,
            erc20_metadata_loads: 0,
        }
    }

    fn insert_kind(&mut self, contract: Address, kind: ContractKind) {
        self.kinds.insert(contract, kind);
    }

    fn insert_fungible_token(&mut self, token: Address) {
        self.insert_kind(token, ContractKind::FungibleLike);
        self.metadata.insert(
            token,
            Erc20Metadata {
                symbol: Some("USDC".to_string()),
                decimals: Some(6),
            },
        );
    }
}

impl DetectionSupport for TestSupport {
    fn resolve_contract_kind(&mut self, contract_address: Address) -> ContractKind {
        self.contract_kind_loads += 1;
        self.kinds
            .get(&contract_address)
            .copied()
            .unwrap_or(ContractKind::Unknown)
    }

    fn load_erc20_metadata(&mut self, token_address: Address) -> Erc20Metadata {
        self.erc20_metadata_loads += 1;
        self.metadata
            .get(&token_address)
            .cloned()
            .unwrap_or_default()
    }
}

struct ForceConsumeDetector;

impl ObservationDetector for ForceConsumeDetector {
    fn detect(
        &self,
        _observation: &Observation,
        _context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        DetectionOutcome::ignored()
    }
}

fn extract_changes(
    status: crate::EvmExecutionStatus,
    observations: Vec<Observation>,
    support: &mut TestSupport,
) -> Vec<Change> {
    ChangeDetectionPipeline::builtin()
        .extract_changes(&execution_artifacts(status, observations), support)
}

fn successful_changes(observations: Vec<Observation>, support: &mut TestSupport) -> Vec<Change> {
    extract_changes(crate::EvmExecutionStatus::Success, observations, support)
}

fn execution_artifacts(
    status: crate::EvmExecutionStatus,
    observations: Vec<Observation>,
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
        observations,
    }
}

fn address(value: &str) -> Address {
    Address::from_str(value).expect("address")
}

fn indexed_address(address: Address) -> B256 {
    address.into_word()
}

fn approval_observation(
    contract_address: Address,
    owner: Address,
    spender: Address,
    value: U256,
) -> Observation {
    Observation::Log {
        address: contract_address,
        topics: vec![
            approval_topic0(),
            indexed_address(owner),
            indexed_address(spender),
        ],
        data: Bytes::from(value.to_be_bytes_vec()),
    }
}

fn approval_for_all_observation(
    contract_address: Address,
    owner: Address,
    operator: Address,
    approved: bool,
) -> Observation {
    Observation::Log {
        address: contract_address,
        topics: vec![
            approval_for_all_topic0(),
            indexed_address(owner),
            indexed_address(operator),
        ],
        data: Bytes::from(U256::from(approved as u8).to_be_bytes_vec()),
    }
}

fn transfer_observation(
    contract_address: Address,
    from: Address,
    to: Address,
    amount: U256,
) -> Observation {
    Observation::Log {
        address: contract_address,
        topics: vec![
            transfer_topic0(),
            indexed_address(from),
            indexed_address(to),
        ],
        data: Bytes::from(amount.to_be_bytes_vec()),
    }
}

fn erc20_asset(contract_address: Address) -> Asset {
    Asset::Erc20 {
        contract_address,
        symbol: Some("USDC".to_string()),
        decimals: Some(6),
        name: None,
    }
}

fn native_transfer_change(from: Address, to: Address, amount: u64) -> Change {
    Change::Transfer(crate::TransferChange {
        asset: Asset::Native {
            symbol: None,
            decimals: None,
        },
        from,
        to,
        amount: Some(U256::from(amount)),
    })
}

fn erc20_transfer_change(token: Address, from: Address, to: Address, amount: u64) -> Change {
    Change::Transfer(crate::TransferChange {
        asset: erc20_asset(token),
        from,
        to,
        amount: Some(U256::from(amount)),
    })
}

fn erc20_approval_change(token: Address, owner: Address, spender: Address, amount: u64) -> Change {
    Change::Approval(ApprovalChange {
        asset: erc20_asset(token),
        owner,
        spender,
        amount: Some(U256::from(amount)),
    })
}

fn erc721_approval_change(
    token: Address,
    owner: Address,
    spender: Address,
    token_id: u64,
) -> Change {
    Change::Approval(ApprovalChange {
        asset: Asset::Erc721 {
            contract_address: token,
            token_id: U256::from(token_id),
            collection_name: None,
            name: None,
            symbol: None,
        },
        owner,
        spender,
        amount: None,
    })
}

fn erc721_collection(contract_address: Address) -> Collection {
    Collection::Erc721 {
        contract_address,
        collection_name: None,
        name: None,
        symbol: None,
    }
}

fn erc1155_collection(contract_address: Address) -> Collection {
    Collection::Erc1155 {
        contract_address,
        collection_name: None,
        name: None,
        symbol: None,
    }
}

fn approval_for_all_change(
    collection: Collection,
    owner: Address,
    operator: Address,
    approved: bool,
) -> Change {
    Change::ApprovalForAll(ApprovalForAllChange {
        collection,
        owner,
        operator,
        approved,
    })
}

#[test]
fn returns_empty_for_failed_execution() {
    let mut support = TestSupport::new();

    let changes = extract_changes(
        crate::EvmExecutionStatus::Failed,
        vec![Observation::NativeTransfer {
            from: Address::ZERO,
            to: Address::repeat_byte(0x11),
            amount: U256::from(1_u64),
        }],
        &mut support,
    );

    assert!(changes.is_empty());
    assert_eq!(support.contract_kind_loads, 0);
    assert_eq!(support.erc20_metadata_loads, 0);
}

#[test]
fn maps_native_and_fungible_transfers_in_observed_order() {
    let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let native_from = address("0x1111111111111111111111111111111111111111");
    let native_to = address("0x2222222222222222222222222222222222222222");
    let token_from = address("0x3333333333333333333333333333333333333333");
    let token_to = address("0x4444444444444444444444444444444444444444");
    let mut support = TestSupport::new();
    support.insert_fungible_token(token);

    let changes = successful_changes(
        vec![
            Observation::NativeTransfer {
                from: native_from,
                to: native_to,
                amount: U256::from(1_u64),
            },
            transfer_observation(token, token_from, token_to, U256::from(2_u64)),
        ],
        &mut support,
    );

    assert_eq!(
        changes,
        vec![
            native_transfer_change(native_from, native_to, 1),
            erc20_transfer_change(token, token_from, token_to, 2),
        ]
    );
}

#[test]
fn maps_approval_changes_for_erc20_and_erc721() {
    let erc20 = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let erc721 = address("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let owner = address("0x1111111111111111111111111111111111111111");
    let spender = address("0x2222222222222222222222222222222222222222");
    let mut support = TestSupport::new();
    support.insert_fungible_token(erc20);
    support.insert_kind(erc721, ContractKind::Erc721);

    let changes = successful_changes(
        vec![
            approval_observation(erc20, owner, spender, U256::from(5_u64)),
            approval_observation(erc721, owner, spender, U256::from(42_u64)),
        ],
        &mut support,
    );

    assert_eq!(
        changes,
        vec![
            erc20_approval_change(erc20, owner, spender, 5),
            erc721_approval_change(erc721, owner, spender, 42),
        ]
    );
}

#[test]
fn maps_approval_for_all_for_erc721_and_erc1155() {
    let erc721 = address("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let erc1155 = address("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let owner = address("0x1111111111111111111111111111111111111111");
    let operator = address("0x2222222222222222222222222222222222222222");
    let mut support = TestSupport::new();
    support.insert_kind(erc721, ContractKind::Erc721);
    support.insert_kind(erc1155, ContractKind::Erc1155);

    let changes = successful_changes(
        vec![
            approval_for_all_observation(erc721, owner, operator, true),
            approval_for_all_observation(erc1155, owner, operator, false),
        ],
        &mut support,
    );

    assert_eq!(
        changes,
        vec![
            approval_for_all_change(erc721_collection(erc721), owner, operator, true),
            approval_for_all_change(erc1155_collection(erc1155), owner, operator, false),
        ]
    );
}

#[test]
fn caches_contract_kinds_and_erc20_metadata_per_address() {
    let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let owner = address("0x1111111111111111111111111111111111111111");
    let spender = address("0x2222222222222222222222222222222222222222");
    let recipient = address("0x3333333333333333333333333333333333333333");
    let mut support = TestSupport::new();
    support.insert_fungible_token(token);

    let changes = successful_changes(
        vec![
            approval_observation(token, owner, spender, U256::from(7_u64)),
            transfer_observation(token, owner, recipient, U256::from(8_u64)),
        ],
        &mut support,
    );

    assert_eq!(
        changes,
        vec![
            erc20_approval_change(token, owner, spender, 7),
            erc20_transfer_change(token, owner, recipient, 8),
        ]
    );
    assert_eq!(support.contract_kind_loads, 1);
    assert_eq!(support.erc20_metadata_loads, 1);
}

#[test]
fn ignores_malformed_approval_for_all_payloads() {
    let collection = address("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let owner = address("0x1111111111111111111111111111111111111111");
    let operator = address("0x2222222222222222222222222222222222222222");
    let mut support = TestSupport::new();
    support.insert_kind(collection, ContractKind::Erc721);

    let malformed = Observation::Log {
        address: collection,
        topics: vec![
            approval_for_all_topic0(),
            indexed_address(owner),
            indexed_address(operator),
        ],
        data: Bytes::from(U256::from(2_u8).to_be_bytes_vec()),
    };

    let changes = successful_changes(vec![malformed], &mut support);
    assert!(changes.is_empty());
}

#[test]
fn contract_specific_detector_can_override_standard_detection() {
    let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let owner = address("0x1111111111111111111111111111111111111111");
    let spender = address("0x2222222222222222222222222222222222222222");
    let mut pipeline = ChangeDetectionPipeline::builtin();
    pipeline.register_contract_detector(None, token, Box::new(ForceConsumeDetector));

    let mut support = TestSupport::new();
    support.insert_fungible_token(token);

    let changes = pipeline.extract_changes(
        &execution_artifacts(
            crate::EvmExecutionStatus::Success,
            vec![approval_observation(
                token,
                owner,
                spender,
                U256::from(9_u64),
            )],
        ),
        &mut support,
    );

    assert!(changes.is_empty());
    assert_eq!(support.contract_kind_loads, 0);
    assert_eq!(support.erc20_metadata_loads, 0);
}
