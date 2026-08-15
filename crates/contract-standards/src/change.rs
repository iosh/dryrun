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
