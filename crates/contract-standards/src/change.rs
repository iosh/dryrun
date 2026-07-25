use alloy_primitives::{Address, U256};

use crate::Position;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardChange {
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
pub struct PositionedStandardChange {
    pub position: Position,
    pub change: StandardChange,
}

impl PositionedStandardChange {
    pub(crate) const fn new(position: Position, change: StandardChange) -> Self {
        Self { position, change }
    }
}
