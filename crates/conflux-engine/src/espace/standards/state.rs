use alloy_primitives::{Address, B256, FixedBytes};
use alloy_sol_types::SolCall;
use cfx_executor::{machine::Machine, state::State};
use cfx_types::AddressSpaceUtil;
use contract_standards::{
    CollectionStandards, ERC165_INTERFACE_ID, ERC721_INTERFACE_ID, ERC1155_INTERFACE_ID,
    Erc20AllowanceCall, Erc20BalanceCall, Erc20TotalSupplyCall, Erc721GetApprovedCall,
    Erc721OwnerCall, Erc721TokenKey, Erc721TokenState, Erc1155BalanceCall,
    INVALID_ERC165_INTERFACE_ID, OperatorApprovalCall, StandardStateValues, StatePhase,
    StateRequirements, SupportsInterfaceCall, validate_collection_standards,
};

use super::read_call::{ReadCallOutcome, execute_read_call};
use crate::{
    ConfluxEngineError,
    execution::PreparedTransactionExecution,
    primitive::{address_to_cfx, b256_from_cfx},
};

pub(crate) fn read_standard_state_values(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    phase: StatePhase,
    requirements: &StateRequirements,
) -> Result<StandardStateValues, ConfluxEngineError> {
    let mut values = StandardStateValues::default();

    for &contract in &requirements.token_contracts {
        values
            .contract_code_hashes
            .insert(contract, read_contract_code_hash(state, phase, contract)?);
    }

    for &collection in &requirements.collection_standards {
        values.collection_standards.insert(
            collection,
            read_collection_standards(state, machine, prepared, phase, collection)?,
        );
    }

    for &key in &requirements.erc20_balances {
        let balance = read_required_value(
            state,
            machine,
            prepared,
            phase,
            key.token,
            Erc20BalanceCall {
                account: key.account,
            },
        )?;
        values.erc20_balances.insert(key, balance);
    }

    for &token in &requirements.erc20_total_supplies {
        let total_supply = read_required_value(
            state,
            machine,
            prepared,
            phase,
            token,
            Erc20TotalSupplyCall {},
        )?;
        values.erc20_total_supplies.insert(token, total_supply);
    }

    for &key in &requirements.erc20_allowances {
        let allowance = read_required_value(
            state,
            machine,
            prepared,
            phase,
            key.token,
            Erc20AllowanceCall {
                owner: key.owner,
                spender: key.spender,
            },
        )?;
        values.erc20_allowances.insert(key, allowance);
    }

    for &key in &requirements.erc721_tokens {
        values.erc721_tokens.insert(
            key,
            read_erc721_token_state(state, machine, prepared, phase, key)?,
        );
    }

    for &key in &requirements.erc1155_balances {
        let balance = read_required_value(
            state,
            machine,
            prepared,
            phase,
            key.collection,
            Erc1155BalanceCall {
                account: key.account,
                id: key.token_id,
            },
        )?;
        values.erc1155_balances.insert(key, balance);
    }

    for &key in &requirements.operator_approvals {
        let approved = read_required_value(
            state,
            machine,
            prepared,
            phase,
            key.collection,
            OperatorApprovalCall {
                owner: key.owner,
                operator: key.operator,
            },
        )?;
        values.operator_approvals.insert(key, approved);
    }

    Ok(values)
}

fn read_required_value<C: SolCall>(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    phase: StatePhase,
    target: Address,
    call: C,
) -> Result<C::Return, ConfluxEngineError> {
    let signature = C::SIGNATURE;
    let output =
        match execute_read_call(state, machine, prepared, target, call.abi_encode().into())? {
            ReadCallOutcome::Success(output) => output,
            ReadCallOutcome::Revert => {
                return Err(analysis_failed(format!(
                    "{phase} required state read {signature} from {target} reverted"
                )));
            }
            ReadCallOutcome::Halt(reason) => {
                return Err(analysis_failed(format!(
                    "{phase} required state read {signature} from {target} halted: {reason}"
                )));
            }
        };

    C::abi_decode_returns_validate(output.as_ref()).map_err(|_| {
        analysis_failed(format!(
            "invalid {phase} return data from {signature} at {target}"
        ))
    })
}

fn read_interface_support(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    phase: StatePhase,
    collection: Address,
    interface_id: [u8; 4],
) -> Result<bool, ConfluxEngineError> {
    read_required_value(
        state,
        machine,
        prepared,
        phase,
        collection,
        SupportsInterfaceCall {
            interfaceId: FixedBytes::from(interface_id),
        },
    )
}

fn read_collection_standards(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    phase: StatePhase,
    collection: Address,
) -> Result<CollectionStandards, ConfluxEngineError> {
    let supports_erc165 = read_interface_support(
        state,
        machine,
        prepared,
        phase,
        collection,
        ERC165_INTERFACE_ID,
    )?;
    if !supports_erc165 {
        return validate_collection_standards(collection, false, false, false, false)
            .map_err(ConfluxEngineError::from);
    }

    let supports_invalid_interface = read_interface_support(
        state,
        machine,
        prepared,
        phase,
        collection,
        INVALID_ERC165_INTERFACE_ID,
    )?;
    if supports_invalid_interface {
        return validate_collection_standards(collection, true, true, false, false)
            .map_err(ConfluxEngineError::from);
    }

    let supports_erc721 = read_interface_support(
        state,
        machine,
        prepared,
        phase,
        collection,
        ERC721_INTERFACE_ID,
    )?;
    let supports_erc1155 = read_interface_support(
        state,
        machine,
        prepared,
        phase,
        collection,
        ERC1155_INTERFACE_ID,
    )?;

    validate_collection_standards(
        collection,
        supports_erc165,
        supports_invalid_interface,
        supports_erc721,
        supports_erc1155,
    )
    .map_err(ConfluxEngineError::from)
}

fn read_contract_code_hash(
    state: &State,
    phase: StatePhase,
    contract: Address,
) -> Result<B256, ConfluxEngineError> {
    let address = address_to_cfx(contract).with_evm_space();
    let (code, code_hash) = state.code_with_hash_on_call(&address).map_err(|error| {
        ConfluxEngineError::StateAccess {
            message: format!("failed to read {phase} token contract {contract}: {error}"),
        }
    })?;

    if code.as_ref().is_none_or(|code| code.is_empty()) {
        return Err(analysis_failed(format!(
            "{phase} token contract {contract} has no runtime code"
        )));
    }

    Ok(b256_from_cfx(code_hash))
}

fn read_erc721_token_state(
    state: &mut State,
    machine: &Machine,
    prepared: &PreparedTransactionExecution,
    phase: StatePhase,
    key: Erc721TokenKey,
) -> Result<Erc721TokenState, ConfluxEngineError> {
    let owner = match execute_read_call(
        state,
        machine,
        prepared,
        key.collection,
        Erc721OwnerCall {
            tokenId: key.token_id,
        }
        .abi_encode()
        .into(),
    )? {
        ReadCallOutcome::Success(output) => {
            Erc721OwnerCall::abi_decode_returns_validate(output.as_ref()).map_err(|_| {
                analysis_failed(format!(
                    "invalid {phase} return data from {} at {}",
                    Erc721OwnerCall::SIGNATURE,
                    key.collection
                ))
            })?
        }
        ReadCallOutcome::Revert => return Ok(Erc721TokenState::OwnerOfReverted),
        ReadCallOutcome::Halt(reason) => {
            return Err(analysis_failed(format!(
                "{phase} required state read {} from {} halted: {reason}",
                Erc721OwnerCall::SIGNATURE,
                key.collection
            )));
        }
    };

    if owner == Address::ZERO {
        return Err(analysis_failed(format!(
            "{phase} {} at {} returned the zero address",
            Erc721OwnerCall::SIGNATURE,
            key.collection
        )));
    }

    let approved_address = read_required_value(
        state,
        machine,
        prepared,
        phase,
        key.collection,
        Erc721GetApprovedCall {
            tokenId: key.token_id,
        },
    )?;

    Ok(Erc721TokenState::Present {
        owner,
        approved_address: (approved_address != Address::ZERO).then_some(approved_address),
    })
}

fn analysis_failed(message: impl Into<String>) -> ConfluxEngineError {
    ConfluxEngineError::Analysis {
        message: message.into(),
    }
}
