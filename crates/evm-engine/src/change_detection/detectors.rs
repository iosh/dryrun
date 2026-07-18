use alloy_primitives::{Address, U256};

use crate::{
    ApprovalChange, ApprovalForAllChange, Asset, BurnChange, Change, Collection, Erc20AssetDisplay,
    MintChange, TransferChange, change_observation::Observation,
};

use super::{
    ContractKind, DetectionContext, DetectionOutcome, ObservationDetector,
    erc721_collection_display,
    log_parsing::{
        extract_approval_for_all_log, extract_erc20_approval_log, extract_erc20_transfer_log,
        extract_erc721_approval_log, extract_erc721_transfer_log,
        extract_erc1155_transfer_batch_log, extract_erc1155_transfer_single_log,
    },
};

pub(super) struct NativeTransferDetector;

impl ObservationDetector for NativeTransferDetector {
    fn detect(
        &self,
        observation: &Observation,
        _context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Some((from, to, amount)) = native_transfer_parts(observation) else {
            return DetectionOutcome::NotHandled;
        };

        DetectionOutcome::handled(Change::Transfer(TransferChange {
            asset: Asset::Native { display: None },
            from,
            to,
            amount: Some(amount),
        }))
    }
}

fn native_transfer_parts(observation: &Observation) -> Option<(Address, Address, U256)> {
    let (from, to, amount) = match observation {
        Observation::Call {
            caller,
            target,
            value,
            ..
        } => (*caller, *target, *value),
        Observation::CreateTransfer { from, to, amount } => (*from, *to, *amount),
        Observation::SelfDestruct {
            contract,
            target,
            amount,
        } => (*contract, *target, *amount),
        Observation::Log { .. } => return None,
    };

    (!amount.is_zero()).then_some((from, to, amount))
}

pub(super) struct StandardTransferDetector;

impl ObservationDetector for StandardTransferDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        if let Some(transfer) = extract_erc20_transfer_log(observation) {
            return DetectionOutcome::handled(classify_standard_transfer(
                erc20_asset(transfer.contract_address, context),
                transfer.from,
                transfer.to,
                Some(transfer.value),
            ));
        }

        let Some(transfer) = extract_erc721_transfer_log(observation) else {
            return DetectionOutcome::NotHandled;
        };

        DetectionOutcome::handled(classify_standard_transfer(
            erc721_asset(transfer.contract_address, transfer.token_id, context),
            transfer.from,
            transfer.to,
            None,
        ))
    }
}

pub(super) struct Erc1155TransferDetector;

impl ObservationDetector for Erc1155TransferDetector {
    fn detect(
        &self,
        observation: &Observation,
        _context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        if let Some(transfer) = extract_erc1155_transfer_single_log(observation) {
            return DetectionOutcome::handled(classify_standard_transfer(
                erc1155_asset(transfer.contract_address, transfer.id),
                transfer.from,
                transfer.to,
                Some(transfer.value),
            ));
        }

        let Some(transfer_batch) = extract_erc1155_transfer_batch_log(observation) else {
            return DetectionOutcome::NotHandled;
        };
        let contract_address = transfer_batch.contract_address;
        let from = transfer_batch.from;
        let to = transfer_batch.to;

        DetectionOutcome::Handled(
            transfer_batch
                .transfers
                .into_iter()
                .map(|entry| {
                    classify_standard_transfer(
                        erc1155_asset(contract_address, entry.id),
                        from,
                        to,
                        Some(entry.value),
                    )
                })
                .collect(),
        )
    }
}

pub(super) struct ApprovalDetector;

impl ObservationDetector for ApprovalDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        if let Some(approval) = extract_erc20_approval_log(observation) {
            return DetectionOutcome::handled(Change::Approval(ApprovalChange {
                asset: erc20_asset(approval.contract_address, context),
                owner: approval.owner,
                spender: approval.spender,
                amount: Some(approval.value),
            }));
        }

        let Some(approval) = extract_erc721_approval_log(observation) else {
            return DetectionOutcome::NotHandled;
        };

        DetectionOutcome::handled(Change::Approval(ApprovalChange {
            asset: erc721_asset(approval.contract_address, approval.token_id, context),
            owner: approval.owner,
            spender: approval.spender,
            amount: None,
        }))
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
            ContractKind::Erc721 => erc721_collection(approval_for_all.contract_address, context),
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
    context: &mut DetectionContext<'_>,
) -> Asset {
    Asset::Erc721 {
        contract_address,
        token_id,
        collection: erc721_collection_display(context.erc721_collection_metadata(contract_address)),
        token: None,
    }
}

fn erc721_collection(contract_address: Address, context: &mut DetectionContext<'_>) -> Collection {
    Collection::Erc721 {
        contract_address,
        collection: erc721_collection_display(context.erc721_collection_metadata(contract_address)),
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
