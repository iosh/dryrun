use alloy::sol_types::SolCall;
use alloy_primitives::{Address, B256, FixedBytes};
use contract_standards::{
    CollectionStandards, ERC165_INTERFACE_ID, ERC721_INTERFACE_ID, ERC1155_INTERFACE_ID,
    Erc20AllowanceCall, Erc20BalanceCall, Erc20TotalSupplyCall, Erc721GetApprovedCall,
    Erc721OwnerCall, Erc721TokenKey, Erc721TokenState, Erc1155BalanceCall,
    INVALID_ERC165_INTERFACE_ID, OperatorApprovalCall, StandardStateValues, StateRequirements,
    SupportsInterfaceCall,
};
use revm::{Database, context_interface::result::EVMError, handler::EvmTr};

use crate::{EvmEngineError, EvmTransaction};

use super::{
    MainnetEvmWithDb,
    read_call::{ReadCallOutcome, execute_read_call, with_read_call_context},
};

pub(super) fn read_token_state_values<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requirements: &StateRequirements,
) -> Result<StandardStateValues, EvmEngineError>
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
) -> Result<StandardStateValues, EvmEngineError>
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
) -> Result<ReadCallOutcome, EvmEngineError>
where
    DB: Database,
    C: SolCall,
{
    let signature = C::SIGNATURE;
    match execute_read_call(evm, transaction, chain_id, target, call.abi_encode().into()) {
        Ok(outcome) => Ok(outcome),
        Err(EVMError::Database(error)) => Err(EvmEngineError::state_access_error(format!(
            "state access failed while reading {} from {target}: {error}",
            signature,
        ))),
        Err(error) => Err(EvmEngineError::analysis_failed(format!(
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
) -> Result<C::Return, EvmEngineError>
where
    DB: Database,
    C: SolCall,
{
    let signature = C::SIGNATURE;
    let outcome = execute_token_state_call(evm, transaction, chain_id, target, call)?;

    let output = match outcome {
        ReadCallOutcome::Success(output) => output,
        ReadCallOutcome::Revert(_) => {
            return Err(EvmEngineError::analysis_failed(format!(
                "required token state read {} from {target} reverted",
                signature,
            )));
        }
        ReadCallOutcome::Halt(reason) => {
            return Err(EvmEngineError::analysis_failed(format!(
                "required token state read {} from {target} halted: {reason}",
                signature,
            )));
        }
    };

    C::abi_decode_returns_validate(output.as_ref()).map_err(|_| {
        EvmEngineError::analysis_failed(
            format!("invalid return data from {signature} at {target}",),
        )
    })
}

fn read_interface_support<DB, INSP>(
    evm: &mut MainnetEvmWithDb<DB, INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    collection: Address,
    interface_id: [u8; 4],
) -> Result<bool, EvmEngineError>
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
) -> Result<CollectionStandards, EvmEngineError>
where
    DB: Database,
{
    let supports_erc165 =
        read_interface_support(evm, transaction, chain_id, collection, ERC165_INTERFACE_ID)?;

    if !supports_erc165 {
        return Err(EvmEngineError::analysis_failed(format!(
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
        return Err(EvmEngineError::analysis_failed(format!(
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
) -> Result<B256, EvmEngineError>
where
    DB: Database,
{
    let database = &mut evm.ctx_mut().journaled_state.database;

    let account = database
        .basic(contract)
        .map_err(|error| {
            EvmEngineError::state_access_error(format!(
                "failed to read token contract {contract}: {error}",
            ))
        })?
        .ok_or_else(|| {
            EvmEngineError::analysis_failed(format!("token contract {contract} does not exist",))
        })?;

    let code_hash = account.code_hash;

    if code_hash == B256::ZERO || account.is_empty_code_hash() {
        return Err(EvmEngineError::analysis_failed(format!(
            "token contract {contract} has no runtime code",
        )));
    }

    let code = match account.code {
        Some(code) => code,
        None => database.code_by_hash(code_hash).map_err(|error| {
            EvmEngineError::state_access_error(format!(
                "failed to read runtime code for token contract {contract}: {error}",
            ))
        })?,
    };

    if code.is_empty() {
        return Err(EvmEngineError::analysis_failed(format!(
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
) -> Result<Erc721TokenState, EvmEngineError>
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
                    EvmEngineError::analysis_failed(format!(
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
                return Err(EvmEngineError::analysis_failed(format!(
                    "required token state read {} from {} halted: {reason}",
                    Erc721OwnerCall::SIGNATURE,
                    key.collection,
                )));
            }
        };

    if owner == Address::ZERO {
        return Err(EvmEngineError::analysis_failed(format!(
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

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, U256};
    use revm::{
        Context, MainBuilder, MainContext,
        database::InMemoryDB,
        state::{AccountInfo, Bytecode, bytecode::opcode},
    };

    use contract_standards::{
        CollectionStandards, Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc721TokenState,
        Erc1155BalanceKey, OperatorApprovalKey, StateRequirements,
    };

    use crate::{EvmTransaction, EvmTransactionVariant};

    use super::read_token_state_values;

    fn erc165_code() -> Bytecode {
        let mut invalid_interface = [0_u8; 32];
        invalid_interface[..4].fill(0xff);

        let mut code = vec![opcode::PUSH1, 0x04, opcode::CALLDATALOAD, opcode::PUSH32];
        code.extend_from_slice(&invalid_interface);
        code.extend_from_slice(&[
            opcode::EQ,
            opcode::ISZERO,
            opcode::PUSH1,
            0x00,
            opcode::MSTORE,
            opcode::PUSH1,
            0x20,
            opcode::PUSH1,
            0x00,
            opcode::RETURN,
        ]);
        Bytecode::new_raw(Bytes::from(code))
    }

    #[test]
    fn reads_state() {
        let caller = Address::repeat_byte(0x01);
        let token_contract = Address::repeat_byte(0x02);
        let reverting_contract = Address::repeat_byte(0x03);
        let account = Address::repeat_byte(0x04);
        let spender = Address::repeat_byte(0x05);
        let operator = Address::repeat_byte(0x06);
        let token_id = U256::from(9_u64);
        let returned_value = U256::from(1_u64);
        let returned_address = Address::with_last_byte(1);

        let token_code = erc165_code();
        let token_code_hash = token_code.hash_slow();
        let reverting_code = Bytecode::new_raw(Bytes::from_static(&[
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::REVERT,
        ]));
        let reverting_code_hash = reverting_code.hash_slow();

        let mut db = InMemoryDB::default();
        db.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000_000_u64)),
        );
        db.insert_account_info(
            token_contract,
            AccountInfo::default().with_nonce(1).with_code(token_code),
        );
        db.insert_account_info(
            reverting_contract,
            AccountInfo::default()
                .with_nonce(1)
                .with_code(reverting_code),
        );

        let erc20_balance_key = Erc20BalanceKey {
            token: token_contract,
            account,
        };
        let allowance_key = Erc20AllowanceKey {
            token: token_contract,
            owner: account,
            spender,
        };
        let present_erc721_key = Erc721TokenKey {
            collection: token_contract,
            token_id,
        };
        let reverted_erc721_key = Erc721TokenKey {
            collection: reverting_contract,
            token_id,
        };
        let erc1155_balance_key = Erc1155BalanceKey {
            collection: token_contract,
            account,
            token_id,
        };
        let operator_approval_key = OperatorApprovalKey {
            collection: token_contract,
            owner: account,
            operator,
        };
        let requirements = StateRequirements {
            token_contracts: vec![token_contract, reverting_contract],
            collection_standards: vec![token_contract],
            erc20_balances: vec![erc20_balance_key],
            erc20_total_supplies: vec![token_contract],
            erc20_allowances: vec![allowance_key],
            erc721_tokens: vec![present_erc721_key, reverted_erc721_key],
            erc1155_balances: vec![erc1155_balance_key],
            operator_approvals: vec![operator_approval_key],
        };
        let transaction = EvmTransaction {
            chain_id: 1,
            from: caller,
            to: Some(token_contract),
            nonce: 0,
            gas_limit: 21_000,
            value: U256::ZERO,
            data: Bytes::new(),
            variant: EvmTransactionVariant::Legacy { gas_price: 0 },
        };
        let mut evm = Context::mainnet().with_db(db).build_mainnet();

        let values = read_token_state_values(&mut evm, &transaction, 1, &requirements)
            .expect("token state values");

        assert_eq!(
            values.contract_code_hashes.get(&token_contract),
            Some(&token_code_hash)
        );
        assert_eq!(
            values.contract_code_hashes.get(&reverting_contract),
            Some(&reverting_code_hash)
        );
        assert_eq!(
            values.collection_standards.get(&token_contract),
            Some(&CollectionStandards {
                supports_erc721: true,
                supports_erc1155: true,
            })
        );
        assert_eq!(
            values.erc20_balances.get(&erc20_balance_key),
            Some(&returned_value)
        );
        assert_eq!(
            values.erc20_total_supplies.get(&token_contract),
            Some(&returned_value)
        );
        assert_eq!(
            values.erc20_allowances.get(&allowance_key),
            Some(&returned_value)
        );
        assert_eq!(
            values.erc721_tokens.get(&present_erc721_key),
            Some(&Erc721TokenState::Present {
                owner: returned_address,
                approved_address: Some(returned_address),
            })
        );
        assert_eq!(
            values.erc721_tokens.get(&reverted_erc721_key),
            Some(&Erc721TokenState::OwnerOfReverted)
        );
        assert_eq!(
            values.erc1155_balances.get(&erc1155_balance_key),
            Some(&returned_value)
        );
        assert_eq!(
            values.operator_approvals.get(&operator_approval_key),
            Some(&true)
        );
    }
}
