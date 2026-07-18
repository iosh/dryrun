use alloy_primitives::{Address, U256};

use crate::{
    ApprovalChange, ApprovalForAllChange, Asset, BurnChange, Change, Collection, Erc20AssetDisplay,
    Erc721CollectionDisplay, MintChange, TransferChange,
};

use super::{
    candidate::{ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence},
    change_data::{ChangeData, ContractKind, Erc20Metadata, Erc721CollectionMetadata},
};

pub(crate) fn build_changes(candidates: Vec<ChangeCandidate>, data: &ChangeData) -> Vec<Change> {
    candidates
        .into_iter()
        .filter_map(|candidate| build_change(candidate.kind, data))
        .collect()
}

fn build_change(kind: ChangeCandidateKind, data: &ChangeData) -> Option<Change> {
    match kind {
        ChangeCandidateKind::NativeTransfer { from, to, amount } => Some(
            classify_standard_transfer(Asset::Native { display: None }, from, to, Some(amount)),
        ),
        ChangeCandidateKind::Erc20Transfer {
            token,
            from,
            to,
            amount,
        } => Some(classify_standard_transfer(
            erc20_asset(token, data.erc20_metadata(token)),
            from,
            to,
            Some(amount),
        )),
        ChangeCandidateKind::Erc721Transfer {
            collection,
            from,
            to,
            token_id,
        } => Some(classify_standard_transfer(
            erc721_asset(
                collection,
                token_id,
                data.erc721_collection_metadata(collection),
            ),
            from,
            to,
            None,
        )),
        ChangeCandidateKind::Erc1155Transfer {
            collection,
            from,
            to,
            token_id,
            amount,
        } => Some(classify_standard_transfer(
            erc1155_asset(collection, token_id),
            from,
            to,
            Some(amount),
        )),
        ChangeCandidateKind::Erc20Allowance {
            token,
            owner,
            spender,
            evidence: Erc20AllowanceEvidence::ApprovalEvent { value },
        } => Some(Change::Approval(ApprovalChange {
            asset: erc20_asset(token, data.erc20_metadata(token)),
            owner,
            spender,
            amount: Some(value),
        })),
        ChangeCandidateKind::Erc20Allowance {
            evidence: Erc20AllowanceEvidence::TransferFromCall { .. },
            ..
        } => None,
        ChangeCandidateKind::Erc721Approval {
            collection,
            owner,
            approved_address,
            token_id,
        } => Some(Change::Approval(ApprovalChange {
            asset: erc721_asset(
                collection,
                token_id,
                data.erc721_collection_metadata(collection),
            ),
            owner,
            spender: approved_address.unwrap_or(Address::ZERO),
            amount: None,
        })),
        ChangeCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        } => match data.contract_kind(collection) {
            ContractKind::Erc721 => Some(Change::ApprovalForAll(ApprovalForAllChange {
                collection: erc721_collection(
                    collection,
                    data.erc721_collection_metadata(collection),
                ),
                owner,
                operator,
                approved,
            })),
            ContractKind::Erc1155 => Some(Change::ApprovalForAll(ApprovalForAllChange {
                collection: erc1155_collection(collection),
                owner,
                operator,
                approved,
            })),
            ContractKind::FungibleLike | ContractKind::Unknown => None,
        },
    }
}

fn erc20_asset(contract_address: Address, metadata: Erc20Metadata) -> Asset {
    let display =
        if metadata.name.is_none() && metadata.symbol.is_none() && metadata.decimals.is_none() {
            None
        } else {
            Some(Erc20AssetDisplay {
                name: metadata.name,
                symbol: metadata.symbol,
                decimals: metadata.decimals,
            })
        };

    Asset::Erc20 {
        contract_address,
        display,
    }
}

fn erc721_asset(
    contract_address: Address,
    token_id: U256,
    metadata: Erc721CollectionMetadata,
) -> Asset {
    Asset::Erc721 {
        contract_address,
        token_id,
        collection: erc721_collection_display(metadata),
        token: None,
    }
}

fn erc721_collection(contract_address: Address, metadata: Erc721CollectionMetadata) -> Collection {
    Collection::Erc721 {
        contract_address,
        collection: erc721_collection_display(metadata),
    }
}

fn erc721_collection_display(
    metadata: Erc721CollectionMetadata,
) -> Option<Erc721CollectionDisplay> {
    if metadata.name.is_none() && metadata.symbol.is_none() {
        None
    } else {
        Some(Erc721CollectionDisplay {
            name: metadata.name,
            symbol: metadata.symbol,
        })
    }
}

fn erc1155_asset(contract_address: Address, token_id: U256) -> Asset {
    Asset::Erc1155 {
        contract_address,
        token_id,
        collection: None,
        token: None,
    }
}

fn erc1155_collection(contract_address: Address) -> Collection {
    Collection::Erc1155 {
        contract_address,
        collection: None,
    }
}

fn classify_standard_transfer(
    asset: Asset,
    from: Address,
    to: Address,
    amount: Option<U256>,
) -> Change {
    if from == Address::ZERO {
        Change::Mint(MintChange { asset, to, amount })
    } else if to == Address::ZERO {
        Change::Burn(BurnChange {
            asset,
            from,
            amount,
        })
    } else {
        Change::Transfer(TransferChange {
            asset,
            from,
            to,
            amount,
        })
    }
}
