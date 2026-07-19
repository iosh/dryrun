use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Erc20Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Erc721CollectionMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        metadata: NativeMetadata,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
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
        metadata: Erc20Metadata,
    },
    Erc721TokenApproval {
        contract_address: Address,
        token_id: U256,
        approved_address_before: Option<Address>,
        approved_address_after: Option<Address>,
        metadata: Erc721CollectionMetadata,
    },
    Erc721OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
        metadata: Erc721CollectionMetadata,
    },
    Erc1155OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
    },
}
