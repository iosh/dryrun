use crate::MetadataKind;
use alloy_primitives::U256;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum StandardChange<A> {
    Erc20Transfer {
        contract_address: A,
        from: A,
        to: A,
        raw_amount: U256,
    },
    Erc20Mint {
        contract_address: A,
        to: A,
        raw_amount: U256,
    },
    Erc20Burn {
        contract_address: A,
        from: A,
        raw_amount: U256,
    },
    Erc20Approval {
        contract_address: A,
        owner: A,
        spender: A,
        before: U256,
        after: U256,
    },
    Erc721Transfer {
        contract_address: A,
        from: A,
        to: A,
        token_id: U256,
    },
    Erc721Mint {
        contract_address: A,
        to: A,
        token_id: U256,
    },
    Erc721Burn {
        contract_address: A,
        from: A,
        token_id: U256,
    },
    Erc721Approval {
        contract_address: A,
        owner: A,
        before: Option<A>,
        after: Option<A>,
        token_id: U256,
    },
    OperatorApproval {
        contract_address: A,
        owner: A,
        operator: A,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract_address: A,
        operator: A,
        from: A,
        to: A,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155MintSingle {
        contract_address: A,
        operator: A,
        to: A,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155BurnSingle {
        contract_address: A,
        operator: A,
        from: A,
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
    Erc1155MintBatch {
        contract_address: A,
        operator: A,
        to: A,
        items: Vec<Erc1155TransferItem>,
    },
    Erc1155BurnBatch {
        contract_address: A,
        operator: A,
        from: A,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Erc1155TransferItem {
    pub token_id: U256,
    pub raw_amount: U256,
}

impl<A> StandardChange<A> {
    pub fn metadata_kind(&self) -> Option<MetadataKind> {
        match self {
            Self::Erc20Transfer { .. }
            | Self::Erc20Mint { .. }
            | Self::Erc20Burn { .. }
            | Self::Erc20Approval { .. } => Some(MetadataKind::Erc20),
            Self::Erc721Transfer { .. }
            | Self::Erc721Mint { .. }
            | Self::Erc721Burn { .. }
            | Self::Erc721Approval { .. } => Some(MetadataKind::Erc721),
            _ => None,
        }
    }
    pub fn try_map_addresses<B, E>(
        self,
        mut map: impl FnMut(A) -> Result<B, E>,
    ) -> Result<StandardChange<B>, E> {
        Ok(match self {
            Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
            } => StandardChange::Erc20Transfer {
                contract_address: map(contract_address)?,
                from: map(from)?,
                to: map(to)?,
                raw_amount,
            },
            Self::Erc20Mint {
                contract_address,
                to,
                raw_amount,
            } => StandardChange::Erc20Mint {
                contract_address: map(contract_address)?,
                to: map(to)?,
                raw_amount,
            },
            Self::Erc20Burn {
                contract_address,
                from,
                raw_amount,
            } => StandardChange::Erc20Burn {
                contract_address: map(contract_address)?,
                from: map(from)?,
                raw_amount,
            },
            Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
            } => StandardChange::Erc20Approval {
                contract_address: map(contract_address)?,
                owner: map(owner)?,
                spender: map(spender)?,
                before,
                after,
            },
            Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
            } => StandardChange::Erc721Transfer {
                contract_address: map(contract_address)?,
                from: map(from)?,
                to: map(to)?,
                token_id,
            },
            Self::Erc721Mint {
                contract_address,
                to,
                token_id,
            } => StandardChange::Erc721Mint {
                contract_address: map(contract_address)?,
                to: map(to)?,
                token_id,
            },
            Self::Erc721Burn {
                contract_address,
                from,
                token_id,
            } => StandardChange::Erc721Burn {
                contract_address: map(contract_address)?,
                from: map(from)?,
                token_id,
            },
            Self::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
            } => StandardChange::Erc721Approval {
                contract_address: map(contract_address)?,
                owner: map(owner)?,
                before: before.map(&mut map).transpose()?,
                after: after.map(&mut map).transpose()?,
                token_id,
            },
            Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            } => StandardChange::OperatorApproval {
                contract_address: map(contract_address)?,
                owner: map(owner)?,
                operator: map(operator)?,
                before,
                after,
            },
            Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => StandardChange::Erc1155TransferSingle {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                from: map(from)?,
                to: map(to)?,
                token_id,
                raw_amount,
            },
            Self::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            } => StandardChange::Erc1155MintSingle {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                to: map(to)?,
                token_id,
                raw_amount,
            },
            Self::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            } => StandardChange::Erc1155BurnSingle {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                from: map(from)?,
                token_id,
                raw_amount,
            },
            Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => StandardChange::Erc1155TransferBatch {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                from: map(from)?,
                to: map(to)?,
                items,
            },
            Self::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items,
            } => StandardChange::Erc1155MintBatch {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                to: map(to)?,
                items,
            },
            Self::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items,
            } => StandardChange::Erc1155BurnBatch {
                contract_address: map(contract_address)?,
                operator: map(operator)?,
                from: map(from)?,
                items,
            },
        })
    }
    pub fn contract_address(&self) -> &A {
        match self {
            Self::Erc20Transfer {
                contract_address, ..
            }
            | Self::Erc20Mint {
                contract_address, ..
            }
            | Self::Erc20Burn {
                contract_address, ..
            }
            | Self::Erc20Approval {
                contract_address, ..
            }
            | Self::Erc721Transfer {
                contract_address, ..
            }
            | Self::Erc721Mint {
                contract_address, ..
            }
            | Self::Erc721Burn {
                contract_address, ..
            }
            | Self::Erc721Approval {
                contract_address, ..
            }
            | Self::OperatorApproval {
                contract_address, ..
            }
            | Self::Erc1155TransferSingle {
                contract_address, ..
            }
            | Self::Erc1155MintSingle {
                contract_address, ..
            }
            | Self::Erc1155BurnSingle {
                contract_address, ..
            }
            | Self::Erc1155TransferBatch {
                contract_address, ..
            }
            | Self::Erc1155MintBatch {
                contract_address, ..
            }
            | Self::Erc1155BurnBatch {
                contract_address, ..
            } => contract_address,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StandardEffect<A> {
    FungibleMovement(A),
    NftMovement { contract: A, token_id: U256 },
    MultiTokenMovement(A),
    Allowance { contract: A, owner: A, spender: A },
    NftApproval { contract: A, token_id: U256 },
    OperatorApproval { contract: A, owner: A, operator: A },
}

impl<A: Clone> StandardChange<A> {
    pub fn effect(&self) -> StandardEffect<A> {
        match self {
            Self::Erc20Transfer {
                contract_address, ..
            }
            | Self::Erc20Mint {
                contract_address, ..
            }
            | Self::Erc20Burn {
                contract_address, ..
            } => StandardEffect::FungibleMovement(contract_address.clone()),
            Self::Erc721Transfer {
                contract_address,
                token_id,
                ..
            }
            | Self::Erc721Mint {
                contract_address,
                token_id,
                ..
            }
            | Self::Erc721Burn {
                contract_address,
                token_id,
                ..
            } => StandardEffect::NftMovement {
                contract: contract_address.clone(),
                token_id: *token_id,
            },
            Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                ..
            } => StandardEffect::Allowance {
                contract: contract_address.clone(),
                owner: owner.clone(),
                spender: spender.clone(),
            },
            Self::Erc721Approval {
                contract_address,
                token_id,
                ..
            } => StandardEffect::NftApproval {
                contract: contract_address.clone(),
                token_id: *token_id,
            },
            Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                ..
            } => StandardEffect::OperatorApproval {
                contract: contract_address.clone(),
                owner: owner.clone(),
                operator: operator.clone(),
            },
            Self::Erc1155TransferSingle {
                contract_address, ..
            }
            | Self::Erc1155TransferBatch {
                contract_address, ..
            }
            | Self::Erc1155MintSingle {
                contract_address, ..
            }
            | Self::Erc1155MintBatch {
                contract_address, ..
            }
            | Self::Erc1155BurnSingle {
                contract_address, ..
            }
            | Self::Erc1155BurnBatch {
                contract_address, ..
            } => StandardEffect::MultiTokenMovement(contract_address.clone()),
        }
    }
}
