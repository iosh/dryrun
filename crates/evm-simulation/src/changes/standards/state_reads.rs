use alloy::sol_types::SolCall;
use alloy_primitives::{Address, B256, FixedBytes};
use contract_standards::legacy::{
    CollectionStandards, ERC165_INTERFACE_ID, ERC721_INTERFACE_ID, ERC1155_INTERFACE_ID,
    Erc20AllowanceCall, Erc20BalanceCall, Erc20TotalSupplyCall, Erc721GetApprovedCall,
    Erc721OwnerCall, Erc721TokenKey, Erc721TokenState, Erc1155BalanceCall,
    INVALID_ERC165_INTERFACE_ID, OperatorApprovalCall, StandardStateValues, StateRequirements,
    SupportsInterfaceCall,
};
use revm::{Database, context_interface::result::EVMError, handler::EvmTr};

use crate::EvmSimulationError;
use simulation_transaction::Transaction as EvmTransaction;

use super::read_call::{ReadCallOutcome, execute_read_call, with_read_call_context};
use crate::execution::MainnetEvmWithDb;

pub(crate) fn read_token_state_values<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requirements: &StateRequirements,
) -> Result<StandardStateValues, EvmSimulationError>
where
    DB: Database,
{
    with_read_call_context(evm, |evm| {
        read_values(evm, transaction, chain_id, requirements)
    })
}

fn read_values<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requirements: &StateRequirements,
) -> Result<StandardStateValues, EvmSimulationError>
where
    DB: Database,
{
    let mut values = StandardStateValues::default();

    for &contract in &requirements.token_contracts {
        values
            .contract_code_hashes
            .insert(contract, read_contract_code_hash(evm, contract)?);
    }

    for &collection in &requirements.collection_standards {
        values.collection_standards.insert(
            collection,
            read_collection_standards(evm, transaction, chain_id, collection)?,
        );
    }

    for &key in &requirements.erc20_balances {
        let balance = read_required_value(
            evm,
            transaction,
            chain_id,
            key.token,
            Erc20BalanceCall {
                account: key.account,
            },
        )?;
        values.erc20_balances.insert(key, balance);
    }

    for &token in &requirements.erc20_total_supplies {
        let total_supply =
            read_required_value(evm, transaction, chain_id, token, Erc20TotalSupplyCall {})?;
        values.erc20_total_supplies.insert(token, total_supply);
    }

    for &key in &requirements.erc20_allowances {
        let allowance = read_required_value(
            evm,
            transaction,
            chain_id,
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
            read_erc721_token_state(evm, transaction, chain_id, key)?,
        );
    }

    for &key in &requirements.erc1155_balances {
        let balance = read_required_value(
            evm,
            transaction,
            chain_id,
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
            evm,
            transaction,
            chain_id,
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

fn execute_token_state_call<DB, INSP, C>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    target: Address,
    call: C,
) -> Result<ReadCallOutcome, EvmSimulationError>
where
    DB: Database,
    C: SolCall,
{
    let signature = C::SIGNATURE;
    match execute_read_call(evm, transaction, chain_id, target, call.abi_encode().into()) {
        Ok(outcome) => Ok(outcome),
        Err(EVMError::Database(error)) => Err(EvmSimulationError::state_access_error(format!(
            "state access failed while reading {} from {target}: {error}",
            signature,
        ))),
        Err(error) => Err(EvmSimulationError::analysis_failed(format!(
            "token state read {} from {target} failed: {error}",
            signature,
        ))),
    }
}

fn read_required_value<DB, INSP, C>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    target: Address,
    call: C,
) -> Result<C::Return, EvmSimulationError>
where
    DB: Database,
    C: SolCall,
{
    let signature = C::SIGNATURE;
    let outcome = execute_token_state_call(evm, transaction, chain_id, target, call)?;

    let output = match outcome {
        ReadCallOutcome::Success(output) => output,
        ReadCallOutcome::Revert(_) => {
            return Err(EvmSimulationError::analysis_failed(format!(
                "required token state read {} from {target} reverted",
                signature,
            )));
        }
        ReadCallOutcome::Halt(reason) => {
            return Err(EvmSimulationError::analysis_failed(format!(
                "required token state read {} from {target} halted: {reason}",
                signature,
            )));
        }
    };

    C::abi_decode_returns_validate(output.as_ref()).map_err(|_| {
        EvmSimulationError::analysis_failed(format!(
            "invalid return data from {signature} at {target}",
        ))
    })
}

fn read_interface_support<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
    interface_id: [u8; 4],
) -> Result<bool, EvmSimulationError>
where
    DB: Database,
{
    read_required_value(
        evm,
        transaction,
        chain_id,
        collection,
        SupportsInterfaceCall {
            interfaceId: FixedBytes::from(interface_id),
        },
    )
}

fn read_collection_standards<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
) -> Result<CollectionStandards, EvmSimulationError>
where
    DB: Database,
{
    let supports_erc165 =
        read_interface_support(evm, transaction, chain_id, collection, ERC165_INTERFACE_ID)?;

    if !supports_erc165 {
        return Err(EvmSimulationError::analysis_failed(format!(
            "token collection {collection} does not support ERC165",
        )));
    }

    let supports_invalid_interface = read_interface_support(
        evm,
        transaction,
        chain_id,
        collection,
        INVALID_ERC165_INTERFACE_ID,
    )?;

    if supports_invalid_interface {
        return Err(EvmSimulationError::analysis_failed(format!(
            "token collection {collection} reports support for the invalid ERC165 interface",
        )));
    }

    let supports_erc721 =
        read_interface_support(evm, transaction, chain_id, collection, ERC721_INTERFACE_ID)?;

    let supports_erc1155 =
        read_interface_support(evm, transaction, chain_id, collection, ERC1155_INTERFACE_ID)?;

    Ok(CollectionStandards {
        supports_erc721,
        supports_erc1155,
    })
}

fn read_contract_code_hash<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    contract: Address,
) -> Result<B256, EvmSimulationError>
where
    DB: Database,
{
    let database = &mut evm.ctx_mut().journaled_state.database;

    let account = database
        .basic(contract)
        .map_err(|error| {
            EvmSimulationError::state_access_error(format!(
                "failed to read token contract {contract}: {error}",
            ))
        })?
        .ok_or_else(|| {
            EvmSimulationError::analysis_failed(
                format!("token contract {contract} does not exist",),
            )
        })?;

    let code_hash = account.code_hash;

    if code_hash == B256::ZERO || account.is_empty_code_hash() {
        return Err(EvmSimulationError::analysis_failed(format!(
            "token contract {contract} has no runtime code",
        )));
    }

    let code = match account.code {
        Some(code) => code,
        None => database.code_by_hash(code_hash).map_err(|error| {
            EvmSimulationError::state_access_error(format!(
                "failed to read runtime code for token contract {contract}: {error}",
            ))
        })?,
    };

    if code.is_empty() {
        return Err(EvmSimulationError::analysis_failed(format!(
            "token contract {contract} has no runtime code",
        )));
    }

    Ok(code_hash)
}

fn read_erc721_token_state<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    key: Erc721TokenKey,
) -> Result<Erc721TokenState, EvmSimulationError>
where
    DB: Database,
{
    let owner_call = Erc721OwnerCall {
        tokenId: key.token_id,
    };

    let owner =
        match execute_token_state_call(evm, transaction, chain_id, key.collection, owner_call)? {
            ReadCallOutcome::Success(output) => {
                Erc721OwnerCall::abi_decode_returns_validate(output.as_ref()).map_err(|_| {
                    EvmSimulationError::analysis_failed(format!(
                        "invalid return data from {} at {}",
                        Erc721OwnerCall::SIGNATURE,
                        key.collection
                    ))
                })?
            }
            ReadCallOutcome::Revert(_) => {
                return Ok(Erc721TokenState::OwnerOfReverted);
            }
            ReadCallOutcome::Halt(reason) => {
                return Err(EvmSimulationError::analysis_failed(format!(
                    "required token state read {} from {} halted: {reason}",
                    Erc721OwnerCall::SIGNATURE,
                    key.collection,
                )));
            }
        };

    if owner == Address::ZERO {
        return Err(EvmSimulationError::analysis_failed(format!(
            "{} at {} returned the zero address",
            Erc721OwnerCall::SIGNATURE,
            key.collection,
        )));
    }

    let approved_address = read_required_value(
        evm,
        transaction,
        chain_id,
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
