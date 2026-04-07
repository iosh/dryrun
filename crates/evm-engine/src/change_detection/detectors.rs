use alloy_primitives::{Address, U256};

use crate::{
    ApprovalChange, ApprovalForAllChange, Asset, Change, Collection, TransferChange,
    change_observation::Observation,
};

use super::{
    ContractKind, DetectionContext, DetectionOutcome, ObservationDetector,
    log_parsing::{extract_approval_for_all_log, extract_approval_log, extract_transfer_log},
};

pub(super) struct NativeTransferDetector;

impl ObservationDetector for NativeTransferDetector {
    fn detect(
        &self,
        observation: &Observation,
        _context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Observation::NativeTransfer { from, to, amount } = observation else {
            return DetectionOutcome::NotHandled;
        };

        DetectionOutcome::handled(Change::Transfer(TransferChange {
            asset: Asset::Native {
                symbol: None,
                decimals: None,
            },
            from: *from,
            to: *to,
            amount: Some(*amount),
        }))
    }
}

pub(super) struct StandardTransferDetector;

impl ObservationDetector for StandardTransferDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Some(transfer) = extract_transfer_log(observation) else {
            return DetectionOutcome::NotHandled;
        };

        // Transfer(address,address,uint256) is shared by ERC20 and ERC721, so
        // the contract kind decides how the log should be interpreted.
        match context.contract_kind(transfer.contract_address) {
            ContractKind::Erc721 | ContractKind::Erc1155 => DetectionOutcome::ignored(),
            ContractKind::FungibleLike | ContractKind::Unknown => {
                DetectionOutcome::handled(Change::Transfer(TransferChange {
                    asset: erc20_asset(transfer.contract_address, context),
                    from: transfer.from,
                    to: transfer.to,
                    amount: Some(transfer.value),
                }))
            }
        }
    }
}

pub(super) struct ApprovalDetector;

impl ObservationDetector for ApprovalDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Some(approval) = extract_approval_log(observation) else {
            return DetectionOutcome::NotHandled;
        };

        // Approval(address,address,uint256) is also shared across ERC20 and
        // ERC721, so the same disambiguation rule applies here.
        match context.contract_kind(approval.contract_address) {
            ContractKind::Erc721 => DetectionOutcome::handled(Change::Approval(ApprovalChange {
                asset: erc721_asset(approval.contract_address, approval.value),
                owner: approval.owner,
                spender: approval.spender,
                amount: None,
            })),
            ContractKind::Erc1155 => DetectionOutcome::ignored(),
            ContractKind::FungibleLike | ContractKind::Unknown => {
                DetectionOutcome::handled(Change::Approval(ApprovalChange {
                    asset: erc20_asset(approval.contract_address, context),
                    owner: approval.owner,
                    spender: approval.spender,
                    amount: Some(approval.value),
                }))
            }
        }
    }
}

pub(super) struct ApprovalForAllDetector;

impl ObservationDetector for ApprovalForAllDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Some(approval_for_all) = extract_approval_for_all_log(observation) else {
            return DetectionOutcome::NotHandled;
        };

        let collection = match context.contract_kind(approval_for_all.contract_address) {
            ContractKind::Erc721 => erc721_collection(approval_for_all.contract_address),
            ContractKind::Erc1155 => erc1155_collection(approval_for_all.contract_address),
            ContractKind::FungibleLike | ContractKind::Unknown => {
                return DetectionOutcome::ignored();
            }
        };

        DetectionOutcome::handled(Change::ApprovalForAll(ApprovalForAllChange {
            collection,
            owner: approval_for_all.owner,
            operator: approval_for_all.operator,
            approved: approval_for_all.approved,
        }))
    }
}

fn erc20_asset(contract_address: Address, context: &mut DetectionContext<'_>) -> Asset {
    let metadata = context.erc20_metadata(contract_address);

    Asset::Erc20 {
        contract_address,
        symbol: metadata.symbol,
        decimals: metadata.decimals,
        name: None,
    }
}

fn erc721_asset(contract_address: Address, token_id: U256) -> Asset {
    Asset::Erc721 {
        contract_address,
        token_id,
        collection_name: None,
        name: None,
        symbol: None,
    }
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
