use alloy_primitives::U256;

use crate::{Erc20Metadata, Erc721CollectionMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardChange<A> {
    Erc20Transfer {
        contract_address: A,
        from: A,
        to: A,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Approval {
        contract_address: A,
        owner: A,
        spender: A,
        approved_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: A,
        from: A,
        to: A,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: A,
        owner: A,
        approved_address: Option<A>,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: A,
        owner: A,
        operator: A,
        approved: bool,
    },
    Erc1155TransferSingle {
        contract_address: A,
        operator: A,
        from: A,
        to: A,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: A,
        operator: A,
        from: A,
        to: A,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Erc1155TransferItem {
    pub token_id: U256,
    pub raw_amount: U256,
}

pub mod legacy {
    use alloy_primitives::{Address, U256};

    use crate::candidate::Position;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Change {
        Erc20Transfer {
            contract_address: Address,
            from: Address,
            to: Address,
            raw_amount: U256,
        },
        Erc20Mint {
            contract_address: Address,
            to: Address,
            raw_amount: U256,
        },
        Erc20Burn {
            contract_address: Address,
            from: Address,
            raw_amount: U256,
        },
        Erc721Transfer {
            contract_address: Address,
            from: Address,
            to: Address,
            token_id: U256,
        },
        Erc721Mint {
            contract_address: Address,
            to: Address,
            token_id: U256,
        },
        Erc721Burn {
            contract_address: Address,
            from: Address,
            token_id: U256,
        },
        Erc1155Transfer {
            contract_address: Address,
            from: Address,
            to: Address,
            token_id: U256,
            raw_amount: U256,
        },
        Erc1155Mint {
            contract_address: Address,
            to: Address,
            token_id: U256,
            raw_amount: U256,
        },
        Erc1155Burn {
            contract_address: Address,
            from: Address,
            token_id: U256,
            raw_amount: U256,
        },
        Erc20Allowance {
            contract_address: Address,
            owner: Address,
            spender: Address,
            raw_amount_before: U256,
            raw_amount_after: U256,
        },
        Erc721TokenApproval {
            contract_address: Address,
            token_id: U256,
            approved_address_before: Option<Address>,
            approved_address_after: Option<Address>,
        },
        Erc721OperatorApproval {
            contract_address: Address,
            owner: Address,
            operator: Address,
            approved_before: bool,
            approved_after: bool,
        },
        Erc1155OperatorApproval {
            contract_address: Address,
            owner: Address,
            operator: Address,
            approved_before: bool,
            approved_after: bool,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PositionedChange {
        pub position: Position,
        pub change: Change,
    }

    impl PositionedChange {
        pub(crate) const fn new(position: Position, change: Change) -> Self {
            Self { position, change }
        }
    }
}
