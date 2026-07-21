use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ObservationPosition {
    pub(super) observation_index: usize,
    pub(super) item_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChangeCandidate {
    pub(super) position: ObservationPosition,
    pub(super) kind: ChangeCandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChangeCandidateKind {
    NativeTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Movement {
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc721Transfer {
        collection: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc1155Transfer {
        collection: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc20Allowance {
        token: Address,
        owner: Address,
        spender: Address,
        evidence: Erc20AllowanceEvidence,
    },
    Erc721Approval {
        collection: Address,
        owner: Address,
        approved_address: Option<Address>,
        token_id: U256,
    },
    OperatorApproval {
        collection: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Erc20AllowanceEvidence {
    ApprovalEvent { value: U256 },
    TransferFromCall { amount: U256 },
}
