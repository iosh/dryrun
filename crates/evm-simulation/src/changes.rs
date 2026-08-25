mod native;
mod standards;
mod wrapped_native;

use std::collections::HashSet;

use alloy::primitives::{Address, Log, U256};
use contract_standards::{Erc20Metadata, MetadataCall, StandardChange, metadata_calls};

use crate::{
    CompleteTransaction, EthereumChainSpec, EvmChangesError, EvmExecutionEvent, EvmSimulationError,
    execution::EvmTransactionExecution,
};

use self::{
    native::analyze_native_changes,
    standards::{DecodedStandardOccurrence, decode_standard_occurrences, load_metadata},
    wrapped_native::{WrappedNativeOccurrence, decode_wrapped_native_occurrences},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmChange {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        raw_amount: U256,
        currency: NativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Standard(StandardChange<Address>),
}

#[derive(Debug)]
struct ChangeOccurrence {
    event_index: usize,
    change: EvmChange,
}

impl ChangeOccurrence {
    const fn new(event_index: usize, change: EvmChange) -> Self {
        Self {
            event_index,
            change,
        }
    }
}

pub(crate) fn analyze_changes(
    output: &mut EvmTransactionExecution,
    transaction: &CompleteTransaction,
    chain_spec: &EthereumChainSpec,
) -> Result<Vec<EvmChange>, EvmSimulationError> {
    let events = output.events().to_vec();
    verify_committed_logs(&events, output.committed_logs())?;

    let currency = chain_spec.native_currency();
    let mut changes = analyze_native_changes(
        output.transition(),
        &events,
        output.caller(),
        output.beneficiary(),
        output.fee_settlement(),
        currency,
    )
    .map_err(EvmChangesError::from)?;

    let wrapped_native_occurrences = chain_spec
        .wrapped_native_token_address()
        .map(|address| decode_wrapped_native_occurrences(&events, address))
        .unwrap_or_default();
    let standard_occurrences = decode_standard_occurrences(&events);
    let calls = collect_metadata_calls(&standard_occurrences, &wrapped_native_occurrences);

    let metadata = output
        .with_post_state_vm(|evm| load_metadata(evm, transaction, chain_spec.chain_id(), calls))?;
    for occurrence in wrapped_native_occurrences {
        let change_metadata = metadata
            .erc20_metadata(&occurrence.contract_address())
            .map_err(EvmChangesError::from)?;
        changes.push(occurrence.into_change(change_metadata));
    }
    for occurrence in standard_occurrences {
        let change = occurrence
            .decoded_log
            .into_change(&metadata)
            .map_err(EvmChangesError::from)?;
        changes.push(ChangeOccurrence::new(
            occurrence.event_index,
            EvmChange::Standard(change),
        ));
    }

    changes.sort_by_key(|occurrence| occurrence.event_index);
    Ok(changes
        .into_iter()
        .map(|occurrence| occurrence.change)
        .collect())
}

fn collect_metadata_calls(
    standard_occurrences: &[DecodedStandardOccurrence],
    wrapped_native_occurrences: &[WrappedNativeOccurrence],
) -> Vec<MetadataCall<Address>> {
    let mut calls = Vec::new();

    for occurrence in standard_occurrences {
        calls.extend(
            metadata_calls(std::iter::once(&occurrence.decoded_log))
                .into_iter()
                .map(|call| (occurrence.event_index, call)),
        );
    }
    for occurrence in wrapped_native_occurrences {
        let event_index = occurrence.event_index();
        let contract_address = occurrence.contract_address();
        calls.extend([
            (event_index, MetadataCall::Name { contract_address }),
            (event_index, MetadataCall::Symbol { contract_address }),
            (event_index, MetadataCall::Decimals { contract_address }),
        ]);
    }

    calls.sort_by_key(|(event_index, _)| *event_index);
    let mut seen = HashSet::new();
    calls
        .into_iter()
        .filter_map(|(_, call)| seen.insert(call.clone()).then_some(call))
        .collect()
}

fn verify_committed_logs(
    events: &[EvmExecutionEvent],
    committed_logs: &[Log],
) -> Result<(), EvmChangesError> {
    let observed_logs = events.iter().filter_map(|event| {
        let EvmExecutionEvent::Log {
            address,
            topics,
            data,
        } = event
        else {
            return None;
        };

        Some((address, topics, data))
    });
    let observed_count = observed_logs.clone().count();

    if observed_count != committed_logs.len() {
        return Err(EvmChangesError::CommittedLogCountMismatch {
            observed_count,
            result_count: committed_logs.len(),
        });
    }

    for (index, ((address, topics, data), committed)) in
        observed_logs.zip(committed_logs).enumerate()
    {
        if *address != committed.address
            || topics.as_slice() != committed.data.topics()
            || data != &committed.data.data
        {
            return Err(EvmChangesError::CommittedLogMismatch { index });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::{
        consensus::{Header, Sealed},
        network::Ethereum,
        primitives::{Address, B256, Bytes, Log, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };
    use revm::{bytecode::Bytecode, state::AccountInfo};

    use super::{analyze_changes, verify_committed_logs};
    use crate::{
        CompleteTransaction, CompleteTransactionVariant, EthereumChainSpec, EvmChange,
        EvmChangesError, EvmExecutionEvent, EvmExecutionObserver, EvmTransactionExecutionResult,
        EvmTransactionExecutor, create_database,
    };

    #[test]
    fn reconciles_observed_logs_with_the_committed_result() {
        let first_address = Address::repeat_byte(1);
        let first_topics = vec![B256::repeat_byte(2)];
        let first_data = Bytes::from_static(&[3]);
        let second_address = Address::repeat_byte(4);
        let second_topics = vec![B256::repeat_byte(5), B256::repeat_byte(6)];
        let second_data = Bytes::from_static(&[7, 8]);
        let events = vec![
            EvmExecutionEvent::Log {
                address: first_address,
                topics: first_topics.clone(),
                data: first_data.clone(),
            },
            EvmExecutionEvent::Call {
                caller: Address::repeat_byte(9),
                target: Address::repeat_byte(10),
                value: U256::from(11),
            },
            EvmExecutionEvent::Log {
                address: second_address,
                topics: second_topics.clone(),
                data: second_data.clone(),
            },
        ];
        let committed_logs = vec![
            Log::new(first_address, first_topics, first_data)
                .expect("first test log should be valid"),
            Log::new(second_address, second_topics, second_data)
                .expect("second test log should be valid"),
        ];

        assert!(verify_committed_logs(&events, &committed_logs).is_ok());
        assert!(matches!(
            verify_committed_logs(&events, &committed_logs[..1]),
            Err(EvmChangesError::CommittedLogCountMismatch {
                observed_count: 2,
                result_count: 1,
            })
        ));

        let mut mismatched_logs = committed_logs;
        mismatched_logs[1].address = Address::repeat_byte(12);
        assert!(matches!(
            verify_committed_logs(&events, &mismatched_logs),
            Err(EvmChangesError::CommittedLogMismatch { index: 1 })
        ));
    }

    #[test]
    fn replays_selfdestruct_to_self_across_the_cancun_boundary() {
        let contract = Address::repeat_byte(2);
        let amount = U256::from(50_000);

        let pre_cancun = execute_selfdestruct_to_self(19_000_000, 1_700_000_000, contract, amount);
        assert!(matches!(
            pre_cancun.as_slice(),
            [EvmChange::SelfDestructBurn {
                contract_address,
                raw_amount,
                ..
            }] if *contract_address == contract && *raw_amount == amount
        ));

        let cancun = execute_selfdestruct_to_self(19_500_000, 1_720_000_000, contract, amount);
        assert!(cancun.is_empty());
    }

    fn execute_selfdestruct_to_self(
        block_number: u64,
        timestamp: u64,
        contract: Address,
        contract_balance: U256,
    ) -> Vec<EvmChange> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");
        let caller = Address::repeat_byte(1);
        let beneficiary = Address::repeat_byte(3);
        let chain_spec = EthereumChainSpec::mainnet();
        let block_hash = B256::repeat_byte(4);
        let header = Header {
            beneficiary,
            number: block_number,
            gas_limit: 30_000_000,
            timestamp,
            mix_hash: B256::repeat_byte(5),
            base_fee_per_gas: Some(1),
            excess_blob_gas: (timestamp >= 1_710_338_135).then_some(0),
            ..Default::default()
        };
        let block = Sealed::new_unchecked(header, block_hash);
        let mut database = create_database(mock_provider(), runtime.handle().clone(), block_hash);
        database.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000)),
        );
        database.insert_account_info(
            contract,
            AccountInfo::default()
                .with_balance(contract_balance)
                .with_nonce(1)
                .with_code(Bytecode::new_legacy(Bytes::from_static(&[0x30, 0xff]))),
        );
        database.insert_account_info(beneficiary, AccountInfo::default());

        let transaction = CompleteTransaction {
            from: caller,
            to: Some(contract),
            nonce: 0,
            gas_limit: 100_000,
            value: U256::ZERO,
            input: Bytes::new(),
            chain_id: 1,
            variant: CompleteTransactionVariant::Legacy { gas_price: 2 },
        };
        let executor =
            EvmTransactionExecutor::new(database, block, &chain_spec, EvmExecutionObserver::new())
                .expect("test block should produce a valid execution environment");
        let EvmTransactionExecutionResult::Executed(output) = executor
            .execute(&transaction)
            .expect("fixture execution should succeed")
        else {
            panic!("fixture transaction should execute");
        };
        let mut output = output.commit().expect("fixture transaction should commit");
        assert!(output.is_success());

        analyze_changes(&mut output, &transaction, &chain_spec)
            .expect("selfdestruct transition should reconcile")
    }

    fn mock_provider() -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(Asserter::new())).erased()
    }
}
