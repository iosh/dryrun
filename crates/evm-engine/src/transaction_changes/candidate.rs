use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ObservationPosition {
    pub(crate) observation_index: usize,
    pub(crate) item_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChangeCandidate {
    pub(crate) position: ObservationPosition,
    pub(crate) kind: ChangeCandidateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeCandidateKind {
    NativeTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Transfer {
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
pub(crate) enum Erc20AllowanceEvidence {
    ApprovalEvent { value: U256 },
    TransferFromCall { amount: U256 },
}
