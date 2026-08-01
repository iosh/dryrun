use cfx_executor::{executive_observer::AddressPocket, machine::Machine};
use cfx_types::{Address, AddressSpaceUtil, Space};
use cfx_vm_types::{CallType, Spec};
use contract_standards::Position;

use super::{
    SponsoredResource, SponsorshipCallOperation, SponsorshipRefundOperation,
    StoragePointConversionOperation,
};
use crate::{
    ConfluxEngineError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

const SET_SPONSOR_FOR_GAS_SELECTOR: [u8; 4] = [0x3e, 0x3e, 0x64, 0x28];
const SET_SPONSOR_FOR_COLLATERAL_SELECTOR: [u8; 4] = [0xe6, 0x6c, 0x1b, 0xea];

pub(super) fn collect_sponsorship_call(
    observations: &[Observation],
    observation_index: usize,
    machine: &Machine,
    spec: &Spec,
) -> Result<Option<(SponsorshipCallOperation, usize)>, ConfluxEngineError> {
    let Some(Observation::Call {
        position,
        space,
        call_type,
        caller,
        target,
        code_address,
        transferred_value,
        input_len,
        input_prefix,
    }) = observations.get(observation_index)
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

    let Some((resource, contract_address)) = decode_sponsorship_call(*input_len, input_prefix)?
    else {
        return Ok(None);
    };
    if *space != Space::Native
        || *call_type != CallType::Call
        || *target != sponsor_contract
        || *code_address != sponsor_contract
    {
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space sponsorship funding call did not use the canonical active internal-contract form",
        ));
    }

    let common = SponsorshipCallFacts {
        call_position: *position,
        sponsor_contract,
        sponsor: *caller,
        contract_address,
        gross_deposit_amount: u256_from_cfx(*transferred_value),
    };
    match resource {
        SponsoredResource::Gas => {
            collect_gas_sponsorship_call(observations, observation_index, common).map(Some)
        }
        SponsoredResource::StorageCollateral => {
            collect_storage_sponsorship_call(observations, observation_index, common).map(Some)
        }
    }
}

pub(super) fn collect_standalone_sponsorship_refund(
    observation: &Observation,
) -> Result<Option<SponsorshipRefundOperation>, ConfluxEngineError> {
    let Observation::InternalTransfer {
        position,
        space,
        from,
        to: AddressPocket::Balance(recipient),
        value,
    } = observation
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
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space standalone sponsorship refund used a non-native balance",
        ));
    }
    let amount = u256_from_cfx(*value);
    Ok(Some(SponsorshipRefundOperation {
        position: Position::new(*position, 0),
        resource,
        sponsor: address_from_cfx(recipient.address),
        contract_address: address_from_cfx(contract_address),
        gross_refund_amount: amount,
        pool_refund_amount: amount,
    }))
}

pub(super) fn collect_storage_point_conversion(
    observations: &[Observation],
    observation_index: usize,
) -> Result<Option<(StoragePointConversionOperation, usize)>, ConfluxEngineError> {
    let Some(first) = observations.get(observation_index) else {
        return Ok(None);
    };
    let Some((position, contract_address, source, amount)) = conversion_transfer(first)? else {
        return Ok(None);
    };
    if amount.is_zero() {
        return Err(ConfluxEngineError::analysis_failed(
            "Core Space storage-point conversion contained a zero transfer",
        ));
    }

    let mut from_sponsor_pool = alloy_primitives::U256::ZERO;
    let mut from_storage_collateral = alloy_primitives::U256::ZERO;
    let mut consumed = 1;
    match source {
        ConversionSource::SponsorPool => {
            from_sponsor_pool = amount;
            if let Some(next) = observation_at_offset(
                observations,
                observation_index,
                1,
                "storage-point conversion",
            )? && let Some((next_position, next_contract, next_source, next_amount)) =
                conversion_transfer(next)?
                && next_contract == contract_address
            {
                if next_source != ConversionSource::StorageCollateral {
                    return Err(ConfluxEngineError::analysis_failed(
                        "Core Space storage-point conversion used an ambiguous source sequence",
                    ));
                }
                verify_next_position(position, next_position, "storage-point conversion")?;
                if next_amount.is_zero() {
                    return Err(ConfluxEngineError::analysis_failed(
                        "Core Space storage-point conversion contained a zero transfer",
                    ));
                }
                from_storage_collateral = next_amount;
                consumed = 2;
            }
        }
        ConversionSource::StorageCollateral => {
            from_storage_collateral = amount;
            if let Some(next) = observation_at_offset(
                observations,
                observation_index,
                1,
                "storage-point conversion",
            )? && let Some((_, next_contract, next_source, _)) = conversion_transfer(next)?
                && next_contract == contract_address
                && next_source == ConversionSource::SponsorPool
            {
                return Err(ConfluxEngineError::analysis_failed(
                    "Core Space storage-point conversion reversed its canonical source order",
                ));
            }
        }
    }

    Ok(Some((
        StoragePointConversionOperation {
            position: Position::new(position, 0),
            contract_address: address_from_cfx(contract_address),
            from_sponsor_pool,
            from_storage_collateral,
        },
        consumed,
    )))
}

#[derive(Clone, Copy)]
struct SponsorshipCallFacts {
    call_position: usize,
    sponsor_contract: Address,
    sponsor: Address,
    contract_address: Address,
    gross_deposit_amount: alloy_primitives::U256,
}

fn collect_gas_sponsorship_call(
    observations: &[Observation],
    observation_index: usize,
    facts: SponsorshipCallFacts,
) -> Result<(SponsorshipCallOperation, usize), ConfluxEngineError> {
    let first = required_transfer(observations, observation_index, 1, "gas sponsorship")?;
    verify_next_position(facts.call_position, first.position, "gas sponsorship")?;

    let (pool_deposit_amount, refund, consumed) =
        if let Some((old_sponsor, pool_refund_amount)) = gas_pool_refund(first, facts)? {
            let deposit = required_transfer(observations, observation_index, 2, "gas sponsorship")?;
            verify_next_position(first.position, deposit.position, "gas sponsorship")?;
            let pool_deposit_amount = gas_pool_deposit(deposit, facts)?;
            if pool_deposit_amount != facts.gross_deposit_amount {
                return Err(transit_mismatch("gas"));
            }
            (
                pool_deposit_amount,
                Some(SponsorshipRefundOperation {
                    position: Position::new(first.position, 0),
                    resource: SponsoredResource::Gas,
                    sponsor: address_from_cfx(old_sponsor),
                    contract_address: address_from_cfx(facts.contract_address),
                    gross_refund_amount: pool_refund_amount,
                    pool_refund_amount,
                }),
                3,
            )
        } else {
            let pool_deposit_amount = gas_pool_deposit(first, facts)?;
            if pool_deposit_amount != facts.gross_deposit_amount {
                return Err(transit_mismatch("gas"));
            }
            (pool_deposit_amount, None, 2)
        };

    Ok((
        SponsorshipCallOperation {
            position: Position::new(facts.call_position, 0),
            resource: SponsoredResource::Gas,
            sponsor: address_from_cfx(facts.sponsor),
            contract_address: address_from_cfx(facts.contract_address),
            gross_deposit_amount: facts.gross_deposit_amount,
            pool_deposit_amount,
            refund,
        },
        consumed,
    ))
}

fn collect_storage_sponsorship_call(
    observations: &[Observation],
    observation_index: usize,
    facts: SponsorshipCallFacts,
) -> Result<(SponsorshipCallOperation, usize), ConfluxEngineError> {
    let first = required_transfer(observations, observation_index, 1, "storage sponsorship")?;
    verify_next_position(facts.call_position, first.position, "storage sponsorship")?;

    let (pool_deposit_amount, refund, consumed) = if let Some((old_sponsor, pool_refund_amount)) =
        storage_pool_refund(first, facts)?
    {
        let compensation =
            required_transfer(observations, observation_index, 2, "storage sponsorship")?;
        verify_next_position(first.position, compensation.position, "storage sponsorship")?;
        let collateral_compensation =
            storage_collateral_compensation(compensation, facts, old_sponsor)?;
        let deposit = required_transfer(observations, observation_index, 3, "storage sponsorship")?;
        verify_next_position(
            compensation.position,
            deposit.position,
            "storage sponsorship",
        )?;
        let pool_deposit_amount = storage_pool_deposit(deposit, facts)?;
        let transit_total = collateral_compensation
            .checked_add(pool_deposit_amount)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage sponsorship transit amount overflowed",
                )
            })?;
        if transit_total != facts.gross_deposit_amount {
            return Err(transit_mismatch("storage"));
        }
        let gross_refund_amount = pool_refund_amount
            .checked_add(collateral_compensation)
            .ok_or_else(|| {
                ConfluxEngineError::analysis_failed(
                    "Core Space storage sponsorship refund amount overflowed",
                )
            })?;
        (
            pool_deposit_amount,
            Some(SponsorshipRefundOperation {
                position: Position::new(first.position, 0),
                resource: SponsoredResource::StorageCollateral,
                sponsor: address_from_cfx(old_sponsor),
                contract_address: address_from_cfx(facts.contract_address),
                gross_refund_amount,
                pool_refund_amount,
            }),
            4,
        )
    } else {
        let pool_deposit_amount = storage_pool_deposit(first, facts)?;
        if pool_deposit_amount != facts.gross_deposit_amount {
            return Err(transit_mismatch("storage"));
        }
        (pool_deposit_amount, None, 2)
    };

    Ok((
        SponsorshipCallOperation {
            position: Position::new(facts.call_position, 0),
            resource: SponsoredResource::StorageCollateral,
            sponsor: address_from_cfx(facts.sponsor),
            contract_address: address_from_cfx(facts.contract_address),
            gross_deposit_amount: facts.gross_deposit_amount,
            pool_deposit_amount,
            refund,
        },
        consumed,
    ))
}

#[derive(Clone, Copy)]
struct TransferFacts<'a> {
    position: usize,
    from: &'a AddressPocket,
    to: &'a AddressPocket,
    amount: alloy_primitives::U256,
}

fn required_transfer<'a>(
    observations: &'a [Observation],
    observation_index: usize,
    offset: usize,
    operation: &str,
) -> Result<TransferFacts<'a>, ConfluxEngineError> {
    let observation = observation_at_offset(observations, observation_index, offset, operation)?;
    let Some(Observation::InternalTransfer {
        position,
        space,
        from,
        to,
        value,
    }) = observation
    else {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} call is missing a contiguous internal-transfer fact"
        )));
    };
    if *space != Space::Native {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} transit used a non-native transfer"
        )));
    }
    Ok(TransferFacts {
        position: *position,
        from,
        to,
        amount: u256_from_cfx(*value),
    })
}

fn observation_at_offset<'a>(
    observations: &'a [Observation],
    observation_index: usize,
    offset: usize,
    operation: &str,
) -> Result<Option<&'a Observation>, ConfluxEngineError> {
    let target_index = observation_index.checked_add(offset).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} observation index overflowed"
        ))
    })?;
    Ok(observations.get(target_index))
}

fn gas_pool_refund(
    transfer: TransferFacts<'_>,
    facts: SponsorshipCallFacts,
) -> Result<Option<(Address, alloy_primitives::U256)>, ConfluxEngineError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::SponsorBalanceForGas(contract_address),
            AddressPocket::Balance(recipient),
        ) if *contract_address == facts.contract_address && recipient.space == Space::Native => {
            Ok(Some((recipient.address, transfer.amount)))
        }
        (AddressPocket::SponsorBalanceForGas(_), AddressPocket::Balance(_)) => {
            Err(transit_mismatch("gas"))
        }
        _ => Ok(None),
    }
}

fn gas_pool_deposit(
    transfer: TransferFacts<'_>,
    facts: SponsorshipCallFacts,
) -> Result<alloy_primitives::U256, ConfluxEngineError> {
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(source), AddressPocket::SponsorBalanceForGas(contract_address))
            if source.space == Space::Native
                && source.address == facts.sponsor_contract
                && *contract_address == facts.contract_address =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("gas")),
    }
}

fn storage_pool_refund(
    transfer: TransferFacts<'_>,
    facts: SponsorshipCallFacts,
) -> Result<Option<(Address, alloy_primitives::U256)>, ConfluxEngineError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::SponsorBalanceForStorage(contract_address),
            AddressPocket::Balance(recipient),
        ) if *contract_address == facts.contract_address && recipient.space == Space::Native => {
            Ok(Some((recipient.address, transfer.amount)))
        }
        (AddressPocket::SponsorBalanceForStorage(_), AddressPocket::Balance(_)) => {
            Err(transit_mismatch("storage"))
        }
        _ => Ok(None),
    }
}

fn storage_collateral_compensation(
    transfer: TransferFacts<'_>,
    facts: SponsorshipCallFacts,
    old_sponsor: Address,
) -> Result<alloy_primitives::U256, ConfluxEngineError> {
    match (transfer.from, transfer.to) {
        (AddressPocket::Balance(source), AddressPocket::Balance(recipient))
            if source.space == Space::Native
                && source.address == facts.sponsor_contract
                && recipient.space == Space::Native
                && recipient.address == old_sponsor =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("storage")),
    }
}

fn storage_pool_deposit(
    transfer: TransferFacts<'_>,
    facts: SponsorshipCallFacts,
) -> Result<alloy_primitives::U256, ConfluxEngineError> {
    match (transfer.from, transfer.to) {
        (
            AddressPocket::Balance(source),
            AddressPocket::SponsorBalanceForStorage(contract_address),
        ) if source.space == Space::Native
            && source.address == facts.sponsor_contract
            && *contract_address == facts.contract_address =>
        {
            Ok(transfer.amount)
        }
        _ => Err(transit_mismatch("storage")),
    }
}

fn decode_sponsorship_call(
    input_len: usize,
    input_prefix: &[u8],
) -> Result<Option<(SponsoredResource, Address)>, ConfluxEngineError> {
    if input_len < 4 || input_prefix.len() < 4 {
        return Ok(None);
    }
    let selector = &input_prefix[..4];
    if selector == SET_SPONSOR_FOR_GAS_SELECTOR {
        require_call_bytes(input_len, input_prefix, 68, "gas sponsorship")?;
        Ok(Some((
            SponsoredResource::Gas,
            Address::from_slice(&input_prefix[16..36]),
        )))
    } else if selector == SET_SPONSOR_FOR_COLLATERAL_SELECTOR {
        require_call_bytes(input_len, input_prefix, 36, "storage sponsorship")?;
        Ok(Some((
            SponsoredResource::StorageCollateral,
            Address::from_slice(&input_prefix[16..36]),
        )))
    } else {
        Ok(None)
    }
}

fn require_call_bytes(
    input_len: usize,
    input_prefix: &[u8],
    required_len: usize,
    operation: &str,
) -> Result<(), ConfluxEngineError> {
    if input_len < required_len || input_prefix.len() < required_len {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} arguments were not fully captured in the call prefix"
        )));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConversionSource {
    SponsorPool,
    StorageCollateral,
}

fn conversion_transfer(
    observation: &Observation,
) -> Result<Option<(usize, Address, ConversionSource, alloy_primitives::U256)>, ConfluxEngineError>
{
    let Observation::InternalTransfer {
        position,
        space,
        from,
        to: AddressPocket::MintBurn,
        value,
    } = observation
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
        return Err(ConfluxEngineError::analysis_failed(
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

fn verify_next_position(
    previous: usize,
    next: usize,
    operation: &str,
) -> Result<(), ConfluxEngineError> {
    let expected = previous.checked_add(1).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} observation position overflowed"
        ))
    })?;
    if next != expected {
        return Err(ConfluxEngineError::analysis_failed(format!(
            "Core Space {operation} internal-transfer facts were not contiguous"
        )));
    }
    Ok(())
}

fn transit_mismatch(resource: &str) -> ConfluxEngineError {
    ConfluxEngineError::analysis_failed(format!(
        "Core Space {resource} sponsorship call had an inconsistent internal-transfer transit"
    ))
}
