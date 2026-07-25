use contract_standards::{PositionedStandardChange, StandardChange};

use crate::{Change, Erc20Metadata, Erc721CollectionMetadata};

use super::PositionedChange;

pub(crate) fn map_contract_changes(
    changes: Vec<PositionedStandardChange>,
) -> Vec<PositionedChange> {
    changes
        .into_iter()
        .map(|positioned| {
            PositionedChange::new(positioned.position, map_standard_change(positioned.change))
        })
        .collect()
}

fn map_standard_change(change: StandardChange) -> Change {
    match change {
        StandardChange::Erc20Transfer {
            contract_address,
            from,
            to,
            raw_amount,
        } => Change::Erc20Transfer {
            contract_address,
            from,
            to,
            raw_amount,
            metadata: Erc20Metadata::default(),
        },
        StandardChange::Erc20Mint {
            contract_address,
            to,
            raw_amount,
        } => Change::Erc20Mint {
            contract_address,
            to,
            raw_amount,
            metadata: Erc20Metadata::default(),
        },
        StandardChange::Erc20Burn {
            contract_address,
            from,
            raw_amount,
        } => Change::Erc20Burn {
            contract_address,
            from,
            raw_amount,
            metadata: Erc20Metadata::default(),
        },
        StandardChange::Erc721Transfer {
            contract_address,
            from,
            to,
            token_id,
        } => Change::Erc721Transfer {
            contract_address,
            from,
            to,
            token_id,
            metadata: Erc721CollectionMetadata::default(),
        },
        StandardChange::Erc721Mint {
            contract_address,
            to,
            token_id,
        } => Change::Erc721Mint {
            contract_address,
            to,
            token_id,
            metadata: Erc721CollectionMetadata::default(),
        },
        StandardChange::Erc721Burn {
            contract_address,
            from,
            token_id,
        } => Change::Erc721Burn {
            contract_address,
            from,
            token_id,
            metadata: Erc721CollectionMetadata::default(),
        },
        StandardChange::Erc1155Transfer {
            contract_address,
            from,
            to,
            token_id,
            raw_amount,
        } => Change::Erc1155Transfer {
            contract_address,
            from,
            to,
            token_id,
            raw_amount,
        },
        StandardChange::Erc1155Mint {
            contract_address,
            to,
            token_id,
            raw_amount,
        } => Change::Erc1155Mint {
            contract_address,
            to,
            token_id,
            raw_amount,
        },
        StandardChange::Erc1155Burn {
            contract_address,
            from,
            token_id,
            raw_amount,
        } => Change::Erc1155Burn {
            contract_address,
            from,
            token_id,
            raw_amount,
        },
        StandardChange::Erc20Allowance {
            contract_address,
            owner,
            spender,
            raw_amount_before,
            raw_amount_after,
        } => Change::Erc20Allowance {
            contract_address,
            owner,
            spender,
            raw_amount_before,
            raw_amount_after,
            metadata: Erc20Metadata::default(),
        },
        StandardChange::Erc721TokenApproval {
            contract_address,
            token_id,
            approved_address_before,
            approved_address_after,
        } => Change::Erc721TokenApproval {
            contract_address,
            token_id,
            approved_address_before,
            approved_address_after,
            metadata: Erc721CollectionMetadata::default(),
        },
        StandardChange::Erc721OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
        } => Change::Erc721OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
            metadata: Erc721CollectionMetadata::default(),
        },
        StandardChange::Erc1155OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
        } => Change::Erc1155OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
        },
    }
}
