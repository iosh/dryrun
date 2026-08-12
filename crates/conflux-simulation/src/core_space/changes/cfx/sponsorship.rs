use crate::core_space::changes::ChangePosition;
use alloy_sol_types::{SolCall, sol};
use cfx_executor::{executive_observer::AddressPocket, machine::Machine};
use cfx_types::{Address, AddressSpaceUtil, Space};
use cfx_vm_types::{CallType, Spec};

use super::{
    PendingSponsorshipEligibilityTarget, SponsoredResource, SponsorshipAccessCallerRole,
    SponsorshipAccessRuleUpdate, SponsorshipFundingOperation, SponsorshipFundingTerms,
    SponsorshipRefundOperation, StoragePointConversionOperation,
};
use crate::{
    core_space::CoreSpaceChangesError,
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{address_from_cfx, address_to_cfx, u256_from_cfx},
};

sol! {
    interface ISponsorWhitelistControlCalls {
        function setSponsorForGas(address contract_address, uint256 upper_bound) external payable;
        function setSponsorForCollateral(address contract_address) external payable;
        function addPrivilege(address[] account_addresses) external;
        function removePrivilege(address[] account_addresses) external;
        function addPrivilegeByAdmin(address contract_address, address[] account_addresses) external;
        function removePrivilegeByAdmin(address contract_address, address[] account_addresses) external;
    }

    interface IAdminControlCalls {
        function setAdmin(address contract_address, address new_admin_address) external;
        function destroy(address contract_address) external;
    }
}

pub(super) enum CollectedSponsorshipCall {
    Funding(Box<SponsorshipFundingOperation>),
    AccessRuleUpdates(Vec<SponsorshipAccessRuleUpdate>),
}

#[derive(Clone, Copy)]
pub(super) struct AdminChangeAttempt {
    pub(super) contract_address: alloy_primitives::Address,
    pub(super) is_destroy: bool,
}

pub(super) fn collect_sponsorship_call(
    trace: &CommittedExecutionTrace,
    frame_position: usize,
    frame_id: FrameId,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<(CollectedSponsorshipCall, Vec<usize>)>, CoreSpaceChangesError> {
    let frame = trace.frame(frame_id);
    let FrameAction::Call {
        call_type,
        caller,
        target,
        code_address,
        transferred_value,
        calldata_len,
        calldata_prefix,
    } = &frame.action
    else {
        return Ok(None);
    };
    let sponsor_contract =
        cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;

    if *target != sponsor_contract && *code_address != sponsor_contract {
        return Ok(None);
    }
    if machine
        .internal_contracts()
        .contract(&sponsor_contract.with_native_space(), spec)
        .is_none()
    {
        return Ok(None);
    }

    let Some(decoded_call) = decode_sponsorship_call(*calldata_len, calldata_prefix)? else {
        return Ok(None);
    };
    if frame.space != Space::Native
        || *call_type != CallType::Call
        || *target != sponsor_contract
        || *code_address != sponsor_contract
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space sponsorship call did not use the canonical active internal-contract form",
        ));
    }

    if decoded_call.must_not_transfer_value() && !transferred_value.is_zero() {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space sponsorship access-rule call transferred a nonzero value",
        ));
    }

    let caller_address = address_from_cfx(*caller);
    if let DecodedSponsorshipCall::AccessRuleUpdates {
        caller_role,
        contract_address,
        account_addresses,
        enabled_after,
    } = decoded_call
    {
        let contract_address = if caller_role == SponsorshipAccessCallerRole::SponsoredContract {
            caller_address
        } else {
            contract_address
        };
        let updates = account_addresses
            .into_iter()
            .enumerate()
            .map(
                |(item_index, account_address)| SponsorshipAccessRuleUpdate {
                    position: ChangePosition::new(frame_position, item_index),
                    caller_role,
                    caller_address,
                    contract_address,
                    account_scope: if account_address.is_zero() {
                        PendingSponsorshipEligibilityTarget::AllAccounts
                    } else {
                        PendingSponsorshipEligibilityTarget::Account(account_address)
                    },
                    enabled_after,
                },
            )
            .collect();
        return Ok(Some((
            CollectedSponsorshipCall::AccessRuleUpdates(updates),
            Vec::new(),
        )));
    }

    let DecodedSponsorshipCall::Funding {
        funding_terms,
        contract_address,
    } = decoded_call
    else {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space sponsorship call classification was inconsistent",
        ));
    };

    let call_context = SponsorshipFundingCallContext {
        frame_position,
        sponsor_contract,
        sponsor: address_to_cfx(caller_address),
        contract_address: address_to_cfx(contract_address),
        gross_deposit_amount: u256_from_cfx(*transferred_value),
    };
    let transfers = funding_transfers(trace, frame_id)?;
    let operation = match funding_terms {
        SponsorshipFundingTerms::Gas {
            gas_fee_upper_bound,
        } => {
            let mut operation = collect_gas_sponsorship_call(&transfers, call_context)?;
            operation.funding_terms = SponsorshipFundingTerms::Gas {
                gas_fee_upper_bound,
            };
            operation
        }
        SponsorshipFundingTerms::StorageCollateral => {
            collect_storage_sponsorship_call(&transfers, call_context)?
        }
    };
    Ok(Some((
        CollectedSponsorshipCall::Funding(Box::new(operation)),
        transfers.iter().map(|transfer| transfer.position).collect(),
    )))
}

pub(super) fn collect_admin_change_attempt(
    trace: &CommittedExecutionTrace,
    frame_id: FrameId,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<AdminChangeAttempt>, CoreSpaceChangesError> {
    let frame = trace.frame(frame_id);
    let FrameAction::Call {
        call_type,
        target,
        code_address,
        transferred_value,
        calldata_len,
        calldata_prefix,
        ..
    } = &frame.action
    else {
        return Ok(None);
    };
    let admin_contract =
        cfx_parameters::internal_contract_addresses::ADMIN_CONTROL_CONTRACT_ADDRESS;
    if *target != admin_contract && *code_address != admin_contract {
        return Ok(None);
    }
    if machine
        .internal_contracts()
        .contract(&admin_contract.with_native_space(), spec)
        .is_none()
    {
        return Ok(None);
    }

    let Some(attempt) = decode_admin_change_call(*calldata_len, calldata_prefix)? else {
        return Ok(None);
    };
    if frame.space != Space::Native
        || *call_type != CallType::Call
        || *target != admin_contract
        || *code_address != admin_contract
        || !transferred_value.is_zero()
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space admin mutation call did not use the canonical active internal-contract form",
        ));
    }
    Ok(Some(attempt))
}

pub(super) fn collect_standalone_sponsorship_refund(
    event: &TraceEvent,
) -> Result<Option<SponsorshipRefundOperation>, CoreSpaceChangesError> {
    let TraceEvent::InternalTransfer {
        position,
        space,
        from,
        to: AddressPocket::Balance(recipient),
        value,
        ..
    } = event
    else {
        return Ok(None);
    };
    let (resource, contract_address) = match from {
        AddressPocket::SponsorBalanceForGas(contract_address) => {
            (SponsoredResource::Gas, *contract_address)
        }
        AddressPocket::SponsorBalanceForStorage(contract_address) => {
            (SponsoredResource::StorageCollateral, *contract_address)
        }
        _ => return Ok(None),
    };
    if *space != Space::Native || recipient.space != Space::Native {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space standalone sponsorship refund used a non-native balance",
        ));
    }
    let amount = u256_from_cfx(*value);
    Ok(Some(SponsorshipRefundOperation {
        position: ChangePosition::new(*position, 0),
        resource,
        sponsor: address_from_cfx(recipient.address),
        contract_address: address_from_cfx(contract_address),
        gross_refund_amount: amount,
        pool_refund_amount: amount,
    }))
}

pub(super) fn collect_storage_point_conversion(
    trace: &CommittedExecutionTrace,
    event: &TraceEvent,
) -> Result<Option<(StoragePointConversionOperation, Vec<usize>)>, CoreSpaceChangesError> {
    let Some((position, contract_address, _, _)) = conversion_transfer(event)? else {
        return Ok(None);
    };

    let mut from_sponsor_pool = alloy_primitives::U256::ZERO;
    let mut from_storage_collateral = alloy_primitives::U256::ZERO;
    let mut sponsor_position = None;
    let mut collateral_position = None;
    let mut transfer_positions = Vec::new();
    for candidate in trace.internal_transfers_in_scope(event.frame_id()) {
        let Some((candidate_position, candidate_contract, source, amount)) =
            conversion_transfer(candidate)?
        else {
            continue;
        };
        if candidate_contract != contract_address {
            continue;
        }
        if amount.is_zero() {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space storage-point conversion contained a zero transfer",
            ));
        }
        transfer_positions.push(candidate_position);
        match source {
            ConversionSource::SponsorPool if sponsor_position.is_none() => {
                sponsor_position = Some(candidate_position);
                from_sponsor_pool = amount;
            }
            ConversionSource::StorageCollateral if collateral_position.is_none() => {
                collateral_position = Some(candidate_position);
                from_storage_collateral = amount;
            }
            ConversionSource::SponsorPool | ConversionSource::StorageCollateral => {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage-point conversion has ambiguous committed internal transfers",
                ));
            }
        }
    }
    if let (Some(sponsor), Some(collateral)) = (sponsor_position, collateral_position)
        && sponsor > collateral
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space storage-point conversion reversed its canonical source order",
        ));
    }

    Ok(Some((
        StoragePointConversionOperation {
            position: ChangePosition::new(position, 0),
            contract_address: address_from_cfx(contract_address),
            from_sponsor_pool,
            from_storage_collateral,
        },
        transfer_positions,
    )))
}

#[derive(Clone, Copy)]
struct SponsorshipFundingCallContext {
    frame_position: usize,
    sponsor_contract: Address,
    sponsor: Address,
    contract_address: Address,
    gross_deposit_amount: alloy_primitives::U256,
}

fn collect_gas_sponsorship_call(
    transfers: &[FundingTransfer<'_>],
    call_context: SponsorshipFundingCallContext,
) -> Result<SponsorshipFundingOperation, CoreSpaceChangesError> {
    let first = required_transfer(transfers, 0, "gas sponsorship")?;

    let (pool_deposit_amount, refund) =
        if let Some((old_sponsor, pool_refund_amount)) = gas_pool_refund(first, call_context)? {
            let deposit = required_transfer(transfers, 1, "gas sponsorship")?;
            require_transfer_count(transfers, 2, "gas sponsorship")?;
            let pool_deposit_amount = gas_pool_deposit(deposit, call_context)?;
            if pool_deposit_amount != call_context.gross_deposit_amount {
                return Err(transit_mismatch("gas"));
            }
            (
                pool_deposit_amount,
                Some(SponsorshipRefundOperation {
                    position: ChangePosition::new(first.position, 0),
                    resource: SponsoredResource::Gas,
                    sponsor: address_from_cfx(old_sponsor),
                    contract_address: address_from_cfx(call_context.contract_address),
                    gross_refund_amount: pool_refund_amount,
                    pool_refund_amount,
                }),
            )
        } else {
            require_transfer_count(transfers, 1, "gas sponsorship")?;
            let pool_deposit_amount = gas_pool_deposit(first, call_context)?;
            if pool_deposit_amount != call_context.gross_deposit_amount {
                return Err(transit_mismatch("gas"));
            }
            (pool_deposit_amount, None)
        };

    Ok(SponsorshipFundingOperation {
        position: ChangePosition::new(call_context.frame_position, 0),
        funding_terms: SponsorshipFundingTerms::Gas {
            gas_fee_upper_bound: alloy_primitives::U256::ZERO,
        },
        sponsor: address_from_cfx(call_context.sponsor),
        contract_address: address_from_cfx(call_context.contract_address),
        gross_deposit_amount: call_context.gross_deposit_amount,
        pool_deposit_amount,
        refund,
    })
}

fn collect_storage_sponsorship_call(
    transfers: &[FundingTransfer<'_>],
    call_context: SponsorshipFundingCallContext,
) -> Result<SponsorshipFundingOperation, CoreSpaceChangesError> {
    let first = required_transfer(transfers, 0, "storage sponsorship")?;

    let (pool_deposit_amount, refund) = if let Some((old_sponsor, pool_refund_amount)) =
        storage_pool_refund(first, call_context)?
    {
        let compensation = required_transfer(transfers, 1, "storage sponsorship")?;
        let collateral_compensation =
            storage_collateral_compensation(compensation, call_context, old_sponsor)?;
        let deposit = required_transfer(transfers, 2, "storage sponsorship")?;
        require_transfer_count(transfers, 3, "storage sponsorship")?;
        let pool_deposit_amount = storage_pool_deposit(deposit, call_context)?;
        let transit_total = collateral_compensation
            .checked_add(pool_deposit_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage sponsorship transit amount overflowed",
                )
            })?;
        if transit_total != call_context.gross_deposit_amount {
            return Err(transit_mismatch("storage"));
        }
        let gross_refund_amount = pool_refund_amount
            .checked_add(collateral_compensation)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(
                    "Core Space storage sponsorship refund amount overflowed",
                )
            })?;
        (
            pool_deposit_amount,
            Some(SponsorshipRefundOperation {
                position: ChangePosition::new(first.position, 0),
                resource: SponsoredResource::StorageCollateral,
                sponsor: address_from_cfx(old_sponsor),
                contract_address: address_from_cfx(call_context.contract_address),
                gross_refund_amount,
                pool_refund_amount,
            }),
        )
    } else {
        require_transfer_count(transfers, 1, "storage sponsorship")?;
        let pool_deposit_amount = storage_pool_deposit(first, call_context)?;
        if pool_deposit_amount != call_context.gross_deposit_amount {
            return Err(transit_mismatch("storage"));
        }
        (pool_deposit_amount, None)
    };

    Ok(SponsorshipFundingOperation {
        position: ChangePosition::new(call_context.frame_position, 0),
        funding_terms: SponsorshipFundingTerms::StorageCollateral,
        sponsor: address_from_cfx(call_context.sponsor),
        contract_address: address_from_cfx(call_context.contract_address),
        gross_deposit_amount: call_context.gross_deposit_amount,
        pool_deposit_amount,
        refund,
    })
}

#[derive(Clone, Copy)]
struct FundingTransfer<'a> {
    position: usize,
    from: &'a AddressPocket,
    to: &'a AddressPocket,
    amount: alloy_primitives::U256,
}

fn required_transfer<'a>(
    transfers: &'a [FundingTransfer<'a>],
    index: usize,
    call_name: &str,
) -> Result<FundingTransfer<'a>, CoreSpaceChangesError> {
    transfers.get(index).copied().ok_or_else(|| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {call_name} call is missing a committed internal transfer"
        ))
    })
}

fn require_transfer_count(
    transfers: &[FundingTransfer<'_>],
    expected: usize,
    call_name: &str,
) -> Result<(), CoreSpaceChangesError> {
    if transfers.len() != expected {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {call_name} call produced {} funding internal transfers, expected {expected}",
            transfers.len()
        )));
    }
    Ok(())
}

fn funding_transfers(
    trace: &CommittedExecutionTrace,
    frame_id: FrameId,
) -> Result<Vec<FundingTransfer<'_>>, CoreSpaceChangesError> {
    trace
        .internal_transfers_in_scope(Some(frame_id))
        .filter_map(|event| match conversion_transfer(event) {
            Ok(Some(_)) => None,
            Ok(None) => Some(funding_transfer(event)),
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn funding_transfer(event: &TraceEvent) -> Result<FundingTransfer<'_>, CoreSpaceChangesError> {
    let TraceEvent::InternalTransfer {
        position,
        space,
        from,
        to,
        value,
        ..
    } = event
    else {
        unreachable!("internal transfer scope only contains internal transfers");
    };
    if *space != Space::Native {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space sponsorship transit used a non-native internal transfer",
        ));
    }
    Ok(FundingTransfer {
        position: *position,
        from,
        to,
        amount: u256_from_cfx(*value),
    })
}

fn gas_pool_refund(
    transfer: FundingTransfer<'_>,
    call_context: SponsorshipFundingCallContext,
) -> Result<Option<(Address, alloy_primitives::U256)>, CoreSpaceChangesError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::SponsorBalanceForGas(contract_address),
            AddressPocket::Balance(recipient),
        ) if *contract_address == call_context.contract_address
            && recipient.space == Space::Native =>
        {
            Ok(Some((recipient.address, transfer.amount)))
        }
        (AddressPocket::SponsorBalanceForGas(_), AddressPocket::Balance(_)) => {
            Err(transit_mismatch("gas"))
        }
        _ => Ok(None),
    }
}

fn gas_pool_deposit(
    transfer: FundingTransfer<'_>,
    call_context: SponsorshipFundingCallContext,
) -> Result<alloy_primitives::U256, CoreSpaceChangesError> {
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(source), AddressPocket::SponsorBalanceForGas(contract_address))
            if source.space == Space::Native
                && source.address == call_context.sponsor_contract
                && *contract_address == call_context.contract_address =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("gas")),
    }
}

fn storage_pool_refund(
    transfer: FundingTransfer<'_>,
    call_context: SponsorshipFundingCallContext,
) -> Result<Option<(Address, alloy_primitives::U256)>, CoreSpaceChangesError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::SponsorBalanceForStorage(contract_address),
            AddressPocket::Balance(recipient),
        ) if *contract_address == call_context.contract_address
            && recipient.space == Space::Native =>
        {
            Ok(Some((recipient.address, transfer.amount)))
        }
        (AddressPocket::SponsorBalanceForStorage(_), AddressPocket::Balance(_)) => {
            Err(transit_mismatch("storage"))
        }
        _ => Ok(None),
    }
}

fn storage_collateral_compensation(
    transfer: FundingTransfer<'_>,
    call_context: SponsorshipFundingCallContext,
    old_sponsor: Address,
) -> Result<alloy_primitives::U256, CoreSpaceChangesError> {
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(source), AddressPocket::Balance(recipient))
            if source.space == Space::Native
                && source.address == call_context.sponsor_contract
                && recipient.space == Space::Native
                && recipient.address == old_sponsor =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("storage")),
    }
}

fn storage_pool_deposit(
    transfer: FundingTransfer<'_>,
    call_context: SponsorshipFundingCallContext,
) -> Result<alloy_primitives::U256, CoreSpaceChangesError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::Balance(source),
            AddressPocket::SponsorBalanceForStorage(contract_address),
        ) if source.space == Space::Native
            && source.address == call_context.sponsor_contract
            && *contract_address == call_context.contract_address =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("storage")),
    }
}

enum DecodedSponsorshipCall {
    Funding {
        funding_terms: SponsorshipFundingTerms,
        contract_address: alloy_primitives::Address,
    },
    AccessRuleUpdates {
        caller_role: SponsorshipAccessCallerRole,
        contract_address: alloy_primitives::Address,
        account_addresses: Vec<alloy_primitives::Address>,
        enabled_after: bool,
    },
}

impl DecodedSponsorshipCall {
    const fn must_not_transfer_value(&self) -> bool {
        matches!(self, Self::AccessRuleUpdates { .. })
    }
}

fn decode_sponsorship_call(
    calldata_len: usize,
    calldata_prefix: &[u8],
) -> Result<Option<DecodedSponsorshipCall>, CoreSpaceChangesError> {
    let Some(selector) = call_selector(calldata_len, calldata_prefix) else {
        return Ok(None);
    };
    let calldata = complete_calldata(calldata_len, calldata_prefix, "sponsorship")?;

    if selector == ISponsorWhitelistControlCalls::setSponsorForGasCall::SELECTOR {
        let call = decode_canonical_call::<ISponsorWhitelistControlCalls::setSponsorForGasCall>(
            calldata,
            "setSponsorForGas",
        )?;
        Ok(Some(DecodedSponsorshipCall::Funding {
            funding_terms: SponsorshipFundingTerms::Gas {
                gas_fee_upper_bound: call.upper_bound,
            },
            contract_address: call.contract_address,
        }))
    } else if selector == ISponsorWhitelistControlCalls::setSponsorForCollateralCall::SELECTOR {
        let call = decode_canonical_call::<
            ISponsorWhitelistControlCalls::setSponsorForCollateralCall,
        >(calldata, "setSponsorForCollateral")?;
        Ok(Some(DecodedSponsorshipCall::Funding {
            funding_terms: SponsorshipFundingTerms::StorageCollateral,
            contract_address: call.contract_address,
        }))
    } else if selector == ISponsorWhitelistControlCalls::addPrivilegeCall::SELECTOR {
        let call = decode_canonical_call::<ISponsorWhitelistControlCalls::addPrivilegeCall>(
            calldata,
            "addPrivilege",
        )?;
        Ok(Some(DecodedSponsorshipCall::AccessRuleUpdates {
            caller_role: SponsorshipAccessCallerRole::SponsoredContract,
            contract_address: alloy_primitives::Address::ZERO,
            account_addresses: call.account_addresses,
            enabled_after: true,
        }))
    } else if selector == ISponsorWhitelistControlCalls::removePrivilegeCall::SELECTOR {
        let call = decode_canonical_call::<ISponsorWhitelistControlCalls::removePrivilegeCall>(
            calldata,
            "removePrivilege",
        )?;
        Ok(Some(DecodedSponsorshipCall::AccessRuleUpdates {
            caller_role: SponsorshipAccessCallerRole::SponsoredContract,
            contract_address: alloy_primitives::Address::ZERO,
            account_addresses: call.account_addresses,
            enabled_after: false,
        }))
    } else if selector == ISponsorWhitelistControlCalls::addPrivilegeByAdminCall::SELECTOR {
        let call = decode_canonical_call::<ISponsorWhitelistControlCalls::addPrivilegeByAdminCall>(
            calldata,
            "addPrivilegeByAdmin",
        )?;
        Ok(Some(DecodedSponsorshipCall::AccessRuleUpdates {
            caller_role: SponsorshipAccessCallerRole::ContractAdmin,
            contract_address: call.contract_address,
            account_addresses: call.account_addresses,
            enabled_after: true,
        }))
    } else if selector == ISponsorWhitelistControlCalls::removePrivilegeByAdminCall::SELECTOR {
        let call = decode_canonical_call::<
            ISponsorWhitelistControlCalls::removePrivilegeByAdminCall,
        >(calldata, "removePrivilegeByAdmin")?;
        Ok(Some(DecodedSponsorshipCall::AccessRuleUpdates {
            caller_role: SponsorshipAccessCallerRole::ContractAdmin,
            contract_address: call.contract_address,
            account_addresses: call.account_addresses,
            enabled_after: false,
        }))
    } else {
        Ok(None)
    }
}

fn decode_admin_change_call(
    calldata_len: usize,
    calldata_prefix: &[u8],
) -> Result<Option<AdminChangeAttempt>, CoreSpaceChangesError> {
    let Some(selector) = call_selector(calldata_len, calldata_prefix) else {
        return Ok(None);
    };
    if selector == IAdminControlCalls::setAdminCall::SELECTOR {
        let calldata = complete_calldata(calldata_len, calldata_prefix, "setAdmin")?;
        let call = decode_canonical_call::<IAdminControlCalls::setAdminCall>(calldata, "setAdmin")?;
        Ok(Some(AdminChangeAttempt {
            contract_address: call.contract_address,
            is_destroy: false,
        }))
    } else if selector == IAdminControlCalls::destroyCall::SELECTOR {
        let calldata = complete_calldata(calldata_len, calldata_prefix, "destroy")?;
        let call = decode_canonical_call::<IAdminControlCalls::destroyCall>(calldata, "destroy")?;
        Ok(Some(AdminChangeAttempt {
            contract_address: call.contract_address,
            is_destroy: true,
        }))
    } else {
        Ok(None)
    }
}

fn call_selector(calldata_len: usize, calldata_prefix: &[u8]) -> Option<[u8; 4]> {
    if calldata_len < 4 || calldata_prefix.len() < 4 {
        return None;
    }
    let mut selector = [0_u8; 4];
    selector.copy_from_slice(&calldata_prefix[..4]);
    Some(selector)
}

fn complete_calldata<'a>(
    calldata_len: usize,
    calldata_prefix: &'a [u8],
    call_name: &str,
) -> Result<&'a [u8], CoreSpaceChangesError> {
    if calldata_prefix.len() != calldata_len {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {call_name} calldata was not fully captured"
        )));
    }
    Ok(calldata_prefix)
}

fn decode_canonical_call<C: SolCall>(
    calldata: &[u8],
    call_name: &str,
) -> Result<C, CoreSpaceChangesError> {
    let call = C::abi_decode_validate(calldata).map_err(|error| {
        CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {call_name} call is not valid ABI data: {error}"
        ))
    })?;
    if call.abi_encode() != calldata {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space {call_name} call is not canonical ABI data"
        )));
    }
    Ok(call)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConversionSource {
    SponsorPool,
    StorageCollateral,
}

fn conversion_transfer(
    event: &TraceEvent,
) -> Result<Option<(usize, Address, ConversionSource, alloy_primitives::U256)>, CoreSpaceChangesError>
{
    let TraceEvent::InternalTransfer {
        position,
        space,
        from,
        to: AddressPocket::MintBurn,
        value,
        ..
    } = event
    else {
        return Ok(None);
    };
    let (contract_address, source) = match from {
        AddressPocket::SponsorBalanceForStorage(contract_address) => {
            (*contract_address, ConversionSource::SponsorPool)
        }
        AddressPocket::StorageCollateral(contract_address) => {
            (*contract_address, ConversionSource::StorageCollateral)
        }
        _ => return Ok(None),
    };
    if *space != Space::Native {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space storage-point conversion used a non-native transfer",
        ));
    }
    Ok(Some((
        *position,
        contract_address,
        source,
        u256_from_cfx(*value),
    )))
}

fn transit_mismatch(resource: &str) -> CoreSpaceChangesError {
    CoreSpaceChangesError::inconsistent_execution(format!(
        "Core Space {resource} sponsorship call had an inconsistent internal-transfer transit"
    ))
}
