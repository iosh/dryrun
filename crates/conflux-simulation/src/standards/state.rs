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

use crate::{
    ConfluxSimulationError,
    execution::PreparedTransactionExecution,
    primitive::{address_to_cfx, b256_from_cfx},
    standards::read_call::{StandardReadCallOutcome, execute_standard_read_call},
};

pub(crate) fn read_standard_state_values(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    state_phase: StatePhase,
    standard_state_requirements: &StateRequirements,
) -> Result<StandardStateValues, ConfluxSimulationError> {
    let analysis_space = prepared_execution.transaction.space();
    let mut standard_state_values = StandardStateValues::default();

    for &token_contract in &standard_state_requirements.token_contracts {
        standard_state_values.contract_code_hashes.insert(
            token_contract,
            read_contract_code_hash(state, state_phase, analysis_space, token_contract)?,
        );
    }

    for &collection in &standard_state_requirements.collection_standards {
        standard_state_values.collection_standards.insert(
            collection,
            read_collection_standards(state, machine, prepared_execution, state_phase, collection)?,
        );
    }

    for &balance_key in &standard_state_requirements.erc20_balances {
        let balance = read_required_value(
            state,
            machine,
            prepared_execution,
            state_phase,
            balance_key.token,
            Erc20BalanceCall {
                account: balance_key.account,
            },
        )?;
        standard_state_values
            .erc20_balances
            .insert(balance_key, balance);
    }

    for &token_contract in &standard_state_requirements.erc20_total_supplies {
        let total_supply = read_required_value(
            state,
            machine,
            prepared_execution,
            state_phase,
            token_contract,
            Erc20TotalSupplyCall {},
        )?;
        standard_state_values
            .erc20_total_supplies
            .insert(token_contract, total_supply);
    }

    for &allowance_key in &standard_state_requirements.erc20_allowances {
        let allowance = read_required_value(
            state,
            machine,
            prepared_execution,
            state_phase,
            allowance_key.token,
            Erc20AllowanceCall {
                owner: allowance_key.owner,
                spender: allowance_key.spender,
            },
        )?;
        standard_state_values
            .erc20_allowances
            .insert(allowance_key, allowance);
    }

    for &token_key in &standard_state_requirements.erc721_tokens {
        standard_state_values.erc721_tokens.insert(
            token_key,
            read_erc721_token_state(state, machine, prepared_execution, state_phase, token_key)?,
        );
    }

    for &balance_key in &standard_state_requirements.erc1155_balances {
        let balance = read_required_value(
            state,
            machine,
            prepared_execution,
            state_phase,
            balance_key.collection,
            Erc1155BalanceCall {
                account: balance_key.account,
                id: balance_key.token_id,
            },
        )?;
        standard_state_values
            .erc1155_balances
            .insert(balance_key, balance);
    }

    for &approval_key in &standard_state_requirements.operator_approvals {
        let approved = read_required_value(
            state,
            machine,
            prepared_execution,
            state_phase,
            approval_key.collection,
            OperatorApprovalCall {
                owner: approval_key.owner,
                operator: approval_key.operator,
            },
        )?;
        standard_state_values
            .operator_approvals
            .insert(approval_key, approved);
    }

    Ok(standard_state_values)
}

fn read_required_value<C: SolCall>(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    state_phase: StatePhase,
    target_contract: Address,
    getter_call: C,
) -> Result<C::Return, ConfluxSimulationError> {
    let getter_signature = C::SIGNATURE;
    let call_return_data = match execute_standard_read_call(
        state,
        machine,
        prepared_execution,
        target_contract,
        getter_call.abi_encode().into(),
    )? {
        StandardReadCallOutcome::Success(return_data) => return_data,
        StandardReadCallOutcome::Revert => {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "{state_phase} required state read {getter_signature} from {target_contract} reverted"
            )));
        }
        StandardReadCallOutcome::Halt(reason) => {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "{state_phase} required state read {getter_signature} from {target_contract} halted: {reason}"
            )));
        }
    };

    C::abi_decode_returns_validate(call_return_data.as_ref()).map_err(|_| {
        ConfluxSimulationError::analysis_failed(format!(
            "invalid {state_phase} return data from {getter_signature} at {target_contract}"
        ))
    })
}

fn read_interface_support(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    state_phase: StatePhase,
    collection: Address,
    interface_id: [u8; 4],
) -> Result<bool, ConfluxSimulationError> {
    read_required_value(
        state,
        machine,
        prepared_execution,
        state_phase,
        collection,
        SupportsInterfaceCall {
            interfaceId: FixedBytes::from(interface_id),
        },
    )
}

fn read_collection_standards(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    state_phase: StatePhase,
    collection: Address,
) -> Result<CollectionStandards, ConfluxSimulationError> {
    let supports_erc165 = read_interface_support(
        state,
        machine,
        prepared_execution,
        state_phase,
        collection,
        ERC165_INTERFACE_ID,
    )?;
    if !supports_erc165 {
        return validate_collection_standards(collection, false, false, false, false)
            .map_err(ConfluxSimulationError::from);
    }

    let supports_invalid_interface = read_interface_support(
        state,
        machine,
        prepared_execution,
        state_phase,
        collection,
        INVALID_ERC165_INTERFACE_ID,
    )?;
    if supports_invalid_interface {
        return validate_collection_standards(collection, true, true, false, false)
            .map_err(ConfluxSimulationError::from);
    }

    let supports_erc721 = read_interface_support(
        state,
        machine,
        prepared_execution,
        state_phase,
        collection,
        ERC721_INTERFACE_ID,
    )?;
    let supports_erc1155 = read_interface_support(
        state,
        machine,
        prepared_execution,
        state_phase,
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
    .map_err(ConfluxSimulationError::from)
}

fn read_contract_code_hash(
    state: &State,
    state_phase: StatePhase,
    contract_space: cfx_types::Space,
    token_contract: Address,
) -> Result<B256, ConfluxSimulationError> {
    let contract_address = address_to_cfx(token_contract).with_space(contract_space);
    let (runtime_code, runtime_code_hash) = state
        .code_with_hash_on_call(&contract_address)
        .map_err(|error| ConfluxSimulationError::StateAccess {
            message: format!(
                "failed to read {state_phase} token contract {token_contract}: {error}"
            ),
        })?;

    if runtime_code.as_ref().is_none_or(|code| code.is_empty()) {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "{state_phase} token contract {token_contract} has no runtime code"
        )));
    }

    Ok(b256_from_cfx(runtime_code_hash))
}

fn read_erc721_token_state(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    state_phase: StatePhase,
    token_key: Erc721TokenKey,
) -> Result<Erc721TokenState, ConfluxSimulationError> {
    let owner = match execute_standard_read_call(
        state,
        machine,
        prepared_execution,
        token_key.collection,
        Erc721OwnerCall {
            tokenId: token_key.token_id,
        }
        .abi_encode()
        .into(),
    )? {
        StandardReadCallOutcome::Success(owner_return_data) => {
            Erc721OwnerCall::abi_decode_returns_validate(owner_return_data.as_ref()).map_err(
                |_| {
                    ConfluxSimulationError::analysis_failed(format!(
                        "invalid {state_phase} return data from {} at {}",
                        Erc721OwnerCall::SIGNATURE,
                        token_key.collection
                    ))
                },
            )?
        }
        StandardReadCallOutcome::Revert => return Ok(Erc721TokenState::OwnerOfReverted),
        StandardReadCallOutcome::Halt(reason) => {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "{state_phase} required state read {} from {} halted: {reason}",
                Erc721OwnerCall::SIGNATURE,
                token_key.collection
            )));
        }
    };

    if owner == Address::ZERO {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "{state_phase} {} at {} returned the zero address",
            Erc721OwnerCall::SIGNATURE,
            token_key.collection
        )));
    }

    let approved_address = read_required_value(
        state,
        machine,
        prepared_execution,
        state_phase,
        token_key.collection,
        Erc721GetApprovedCall {
            tokenId: token_key.token_id,
        },
    )?;

    Ok(Erc721TokenState::Present {
        owner,
        approved_address: (approved_address != Address::ZERO).then_some(approved_address),
    })
}
