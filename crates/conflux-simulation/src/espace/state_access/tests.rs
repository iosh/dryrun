use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use alloy::{
    providers::{Provider, RootProvider},
    rpc::client::RpcClient,
    transports::{TransportError, TransportFut},
};
use alloy_json_rpc::{RequestPacket, Response, ResponsePacket, ResponsePayload, SerializedRequest};
use alloy_primitives::{Address, B256, Bytes, U256, hex, keccak256};
use cfx_types::{AddressSpaceUtil, H256, Space, U256 as CfxU256};
use conflux_provider::{ConfluxProvider, CoreAddress, Network};
use primitives::transaction::{Action, Eip155Transaction, EthereumTransaction};
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tower::Service;

use super::{EspaceReadCallOutcome, EspaceSimulationLimits, EspaceStateAccess};
use crate::{
    chain_spec::ConfluxChainSpec,
    espace::{EspaceExecutedTransaction, EspaceExecutionStatus, EspaceFrameAction},
    execution::{
        ConfluxTransactionExecutor, CoreSpacePivotBlockContext, DryRunTransactionInput,
        EspaceTransactionInput, ExecutionConsensusContext, ExecutionTraceObserver, LogCheckpoint,
        TransactionExecutionInput, build_conflux_state, build_espace_execution_block_context,
        build_execution_block_context,
    },
    primitive::{address_to_cfx, b256_to_cfx},
    state::{ConfluxSimulationProvider, ConfluxStateAnchor, ConfluxStateSource, EspaceRpcBlock},
};

const SENDER: Address = Address::repeat_byte(0x11);
const CONTRACT: Address = Address::repeat_byte(0x22);
const CHILD: Address = Address::repeat_byte(0x33);
const PIVOT_HASH: B256 = B256::repeat_byte(0x99);
const EPOCH: u64 = 100_000_000;
const SENDER_NONCE: u64 = 7;
const LIMITS: EspaceSimulationLimits = EspaceSimulationLimits::new(3, 64, 16, 500_000, 1024);

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_snapshots_preserve_state_and_follow_nested_and_transaction_rollback() {
    // The successful child returns 7. Its parent emits two more logs, exceeding
    // the checkpoint limit, then reverts with the child's return data.
    let child_code = hex!("6002600055600160006000a1600760005260206000f3");
    let mut parent_code = Vec::new();
    append_call(&mut parent_code, CHILD, 0);
    parent_code.extend(hex!("6003600055600160006000a16004600055600160006000a160206000fd"));

    for revert_transaction in [false, true] {
        // The constructor logs slot 0 at 1 and 9, then changes it to 11.
        // The second log carries the reverted parent's return data, proving
        // execution reached the deliberate REVERT after the child succeeded.
        let mut init_code = hex!("6001600055600160006000a1").to_vec();
        append_call(&mut init_code, CONTRACT, 0);
        init_code.extend(hex!("6009600055600160206000a1600b600055"));
        if revert_transaction {
            init_code.extend(hex!("60006000fd"));
        } else {
            init_code.push(0x00);
        }

        let created = SENDER.create(SENDER_NONCE);
        let rpc = StateRpc {
            accounts: HashMap::from([
                (SENDER, sender_account()),
                (created, RpcAccount::default()),
                (
                    CONTRACT,
                    RpcAccount {
                        code: parent_code.clone().into(),
                        ..Default::default()
                    },
                ),
                (
                    CHILD,
                    RpcAccount {
                        code: Bytes::copy_from_slice(&child_code),
                        ..Default::default()
                    },
                ),
            ]),
            storage: HashMap::from([
                ((CONTRACT, U256::ZERO), U256::ZERO),
                ((CHILD, U256::ZERO), U256::ZERO),
            ]),
            ..Default::default()
        };
        let source = rpc.source().await;

        tokio::task::spawn_blocking(move || {
            let (record, state) = execute(source, Action::Create, init_code.clone());
            let occurrences = record.semantic_log_occurrences().unwrap().collect::<Vec<_>>();

            if revert_transaction {
                assert_eq!(record.status(), EspaceExecutionStatus::Reverted);
                assert!(record.committed_frames().is_empty());
                assert!(record.committed_logs().is_empty());
                assert!(occurrences.is_empty());
                assert_eq!(
                    state.finalized().storage_word(created, B256::ZERO).unwrap(),
                    B256::ZERO,
                );
                return;
            }

            assert_eq!(record.status(), EspaceExecutionStatus::Success);
            assert_eq!(record.committed_frames().len(), 1);
            assert!(matches!(
                record.committed_frames()[0].action(),
                EspaceFrameAction::Create { actual_address, init_code: recorded_code, .. }
                    if *actual_address == created && recorded_code.as_ref() == init_code.as_slice()
            ));
            assert_eq!(record.committed_logs().len(), 2);
            assert_eq!(occurrences.len(), 2);
            assert!(occurrences[0].position() < occurrences[1].position());
            assert!(
                occurrences
                    .iter()
                    .all(|occurrence| occurrence.log().address() == created)
            );
            assert_eq!(occurrences[1].log().data().as_ref(), word(7).as_slice());

            assert_eq!(
                state.initial().storage_word(created, B256::ZERO).unwrap(),
                B256::ZERO,
            );
            for (occurrence, expected) in occurrences.iter().zip([1, 9]) {
                assert_eq!(
                    state
                        .at(occurrence.handle())
                        .unwrap()
                        .storage_word(created, B256::ZERO)
                        .unwrap(),
                    word(expected),
                );
            }
            let around = state.around(occurrences[1].handle()).unwrap();
            assert_eq!(
                around.previous().storage_word(created, B256::ZERO).unwrap(),
                word(1),
            );
            assert_eq!(
                around.current().storage_word(created, B256::ZERO).unwrap(),
                word(9),
            );
            assert_eq!(
                state.finalized().storage_word(created, B256::ZERO).unwrap(),
                word(11),
            );
            for address in [CONTRACT, CHILD] {
                assert_eq!(
                    state.finalized().storage_word(address, B256::ZERO).unwrap(),
                    B256::ZERO,
                );
            }
        })
        .await
        .expect("snapshot execution task panicked");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anchored_read_calls_leave_storage_balances_and_nonce_unchanged() {
    // Return the original slot value, increment it and log, transfer 3 to the
    // caller, then increment again. These are real writes, not a static call.
    let mut code = hex!("600054600052600054600101600055600160006000a1").to_vec();
    append_call(&mut code, SENDER, 3);
    code.extend(hex!("60005460010160005560206000f3"));
    let rpc = StateRpc {
        accounts: HashMap::from([
            (SENDER, sender_account()),
            (
                CONTRACT,
                RpcAccount {
                    balance: 100,
                    nonce: 1,
                    code: code.into(),
                },
            ),
        ]),
        storage: HashMap::from([
            ((CONTRACT, U256::ZERO), U256::from(5)),
            ((CONTRACT, U256::from(1)), U256::from(42)),
        ]),
        ..Default::default()
    };
    let source = rpc.source().await;

    tokio::task::spawn_blocking(move || {
        let (record, state) = execute(source, Action::Call(address_to_cfx(CONTRACT)), Vec::new());
        assert_eq!(record.status(), EspaceExecutionStatus::Success);
        let occurrences = record.semantic_log_occurrences().unwrap().collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 1);
        let at_log = state.at(occurrences[0].handle()).unwrap();

        for (reader, expected_storage, expected_balance, expected_nonce) in [
            (state.initial(), 5_u64, 100_u64, SENDER_NONCE),
            (at_log, 6, 100, 8),
            (state.finalized(), 7, 97, 8),
        ] {
            // Read State directly so the reader's result caches cannot hide a
            // failed restore. Include both sides of the native transfer.
            let raw_state = || {
                let state = reader.state.borrow();
                let target = address_to_cfx(CONTRACT).with_evm_space();
                let sender = address_to_cfx(SENDER).with_evm_space();
                (
                    state.storage_at(&target, &[0; 32]).unwrap(),
                    state.balance(&target).unwrap(),
                    state.balance(&sender).unwrap(),
                    state.nonce(&sender).unwrap(),
                )
            };
            let before = raw_state();
            assert_eq!(before.0, CfxU256::from(expected_storage));
            assert_eq!(before.1, CfxU256::from(expected_balance));
            assert_eq!(before.3, CfxU256::from(expected_nonce));

            // Different calldata bypasses read-call memoization on the second
            // probe; both executions must start from the same state point.
            for input in [1, 2] {
                assert_eq!(
                    reader.read_call(CONTRACT, Bytes::from(vec![input])).unwrap(),
                    EspaceReadCallOutcome::Success(Bytes::copy_from_slice(
                        word(expected_storage).as_slice(),
                    )),
                );
                assert_eq!(raw_state(), before);
            }

            // Slot 1 was never touched by execution or the probes. Each state
            // point must fetch it lazily from the original fixed block hash.
            assert_eq!(reader.storage_word(CONTRACT, word(1)).unwrap(), word(42));
        }
    })
    .await
    .expect("read-call execution task panicked");

    let requests = rpc.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|(method, params)| method == "eth_getStorageAt" && params[1] == "0x1")
            .count(),
        3,
        "initial, log and finalized readers must each exercise anchored lazy storage access",
    );
}

fn execute(
    source: Arc<ConfluxStateSource>,
    action: Action,
    data: Vec<u8>,
) -> (EspaceExecutedTransaction, EspaceStateAccess) {
    let handle = Handle::current();
    let mut execution_state = build_conflux_state(Arc::clone(&source), handle.clone()).unwrap();
    let machine = Arc::new(ConfluxChainSpec::mainnet().build_machine());
    // A fixed mainnet context after eSpace activation and before CIP-1559.
    let pivot = CoreSpacePivotBlockContext {
        block_number: 240_000_000,
        epoch_height: EPOCH,
        author: address_to_cfx(SENDER),
        timestamp: 1_700_000_000,
        hash: b256_to_cfx(PIVOT_HASH),
        base_fee_per_gas: None,
    };
    let espace = build_espace_execution_block_context(&EspaceRpcBlock {
        hash: PIVOT_HASH,
        number: EPOCH,
        base_fee_per_gas: None,
    });
    let input = TransactionExecutionInput {
        block_context: build_execution_block_context(
            &pivot,
            &espace,
            ExecutionConsensusContext::default(),
        ),
        transaction: DryRunTransactionInput::Espace(EspaceTransactionInput {
            sender: address_to_cfx(SENDER),
            tx: EthereumTransaction::Eip155(Eip155Transaction {
                nonce: CfxU256::from(SENDER_NONCE),
                gas_price: CfxU256::from(1),
                gas: CfxU256::from(1_000_000),
                action,
                value: CfxU256::zero(),
                chain_id: Some(1030),
                data,
            }),
        }),
    };
    let observer = ExecutionTraceObserver::new(Space::Ethereum).with_log_checkpoints(
        vec![LogCheckpoint {
            space: Space::Ethereum,
            address: None,
            topic0: H256::from_low_u64_be(1),
        }],
        LIMITS.max_occurrence_checkpoints,
    );
    let mut execution = ConfluxTransactionExecutor::new(&mut execution_state, &machine)
        .execute(input, observer)
        .unwrap();
    let mut state = EspaceStateAccess::new(
        source,
        handle,
        execution_state,
        machine,
        &execution.prepared,
        SENDER,
        LIMITS,
    )
    .unwrap();
    let record = EspaceExecutedTransaction::from_outcome(&mut execution.outcome, &mut state).unwrap();
    (record, state)
}

// CALL with no input and a 32-byte return buffer at memory offset zero.
fn append_call(code: &mut Vec<u8>, target: Address, value: u8) {
    code.extend([0x60, 0x20, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, value, 0x73]);
    code.extend_from_slice(target.as_slice());
    code.extend([0x5a, 0xf1, 0x50]); // GAS, CALL, POP
}

fn word(value: u64) -> B256 {
    B256::from(U256::from(value).to_be_bytes::<32>())
}

#[derive(Clone, Default)]
struct RpcAccount {
    balance: u64,
    nonce: u64,
    code: Bytes,
}

fn sender_account() -> RpcAccount {
    RpcAccount {
        balance: 1_000_000_000,
        nonce: SENDER_NONCE,
        code: Bytes::new(),
    }
}

#[derive(Clone, Default)]
struct StateRpc {
    accounts: HashMap<Address, RpcAccount>,
    storage: HashMap<(Address, U256), U256>,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
}

impl StateRpc {
    async fn source(&self) -> Arc<ConfluxStateSource> {
        let client = RpcClient::new(self.clone(), true);
        let provider = ConfluxSimulationProvider::new(
            RootProvider::new(client.clone()).erased(),
            ConfluxProvider::new(client),
            Network::Main,
        );
        Arc::new(
            ConfluxStateSource::prepare(
                ConfluxStateAnchor::new(EPOCH, b256_to_cfx(PIVOT_HASH)),
                provider,
            )
            .await
            .unwrap(),
        )
    }

    fn respond(&self, request: SerializedRequest) -> Response {
        let method = request.method();
        let params: Value = serde_json::from_str(request.params().unwrap().get()).unwrap();
        self.requests
            .lock()
            .unwrap()
            .push((method.to_owned(), params.clone()));
        let result = match method {
            "eth_getBalance" | "eth_getTransactionCount" | "eth_getCode" | "eth_getStorageAt" => {
                let args = params.as_array().unwrap();
                assert_eq!(
                    args.last().unwrap(),
                    &json!({ "blockHash": PIVOT_HASH, "requireCanonical": true }),
                    "wrong anchor for {method}",
                );
                let address: Address = serde_json::from_value(args[0].clone()).unwrap();
                let account = self.accounts.get(&address).expect("unexpected eSpace account read");
                if method == "eth_getStorageAt" {
                    assert_eq!(args.len(), 3);
                    let slot: U256 = serde_json::from_value(args[1].clone()).unwrap();
                    let value = self
                        .storage
                        .get(&(address, slot))
                        .expect("unexpected eSpace storage read");
                    json!(B256::from(value.to_be_bytes::<32>()))
                } else {
                    assert_eq!(args.len(), 2);
                    match method {
                        "eth_getBalance" => json!(U256::from(account.balance)),
                        "eth_getTransactionCount" => json!(U256::from(account.nonce)),
                        "eth_getCode" => json!(account.code),
                        _ => unreachable!(),
                    }
                }
            }
            "cfx_getAccount" | "cfx_getCollateralForStorage" => {
                let sender = CoreAddress::from_bytes([0x11; 20], Network::Main).unwrap();
                assert_eq!(params, json!([sender, format!("{EPOCH:#x}")]));
                if method == "cfx_getCollateralForStorage" {
                    json!("0x0")
                } else {
                    json!({
                        "address": sender,
                        "balance": "0x0",
                        "nonce": "0x0",
                        "codeHash": keccak256([]),
                        "stakingBalance": "0x0",
                        "collateralForStorage": "0x0",
                        "accumulatedInterestReturn": "0x0",
                        "admin": CoreAddress::from_bytes([0; 20], Network::Main).unwrap(),
                    })
                }
            }
            _ => {
                assert_eq!(
                    params,
                    json!([format!("{EPOCH:#x}")]),
                    "wrong epoch for {method}",
                );
                match method {
                    "cfx_getInterestRate" | "cfx_getFeeBurnt" => json!("0x0"),
                    "cfx_getAccumulateInterestRate" => json!("0x1"),
                    "cfx_getSupplyInfo" => json!({
                        "totalCirculating": "0x10000000000",
                        "totalIssued": "0x10000000000",
                        "totalStaking": "0x0",
                        "totalCollateral": "0x0",
                        "totalEspaceTokens": "0x10000000000",
                    }),
                    "cfx_getCollateralInfo" => json!({
                        "totalStorageTokens": "0x0",
                        "convertedStoragePoints": "0x0",
                        "usedStoragePoints": "0x0",
                    }),
                    "cfx_getPoSEconomics" => json!({
                        "totalPosStakingTokens": "0x0",
                        "distributablePosInterest": "0x0",
                        "lastDistributeBlock": "0x0",
                    }),
                    "cfx_getParamsFromVote" => json!({
                        "powBaseReward": "0x0",
                        "interestRate": "0x0",
                        "storagePointProp": "0x0",
                        "baseFeeShareProp": "0x0",
                    }),
                    _ => panic!("unexpected RPC method {method}"),
                }
            }
        };
        Response {
            id: request.id().clone(),
            payload: ResponsePayload::Success(serde_json::value::to_raw_value(&result).unwrap()),
        }
    }
}

impl Service<RequestPacket> for StateRpc {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: RequestPacket) -> Self::Future {
        let response = match request {
            RequestPacket::Single(request) => ResponsePacket::Single(self.respond(request)),
            RequestPacket::Batch(requests) => ResponsePacket::Batch(
                requests
                    .into_iter()
                    .map(|request| self.respond(request))
                    .collect(),
            ),
        };
        Box::pin(async move { Ok(response) })
    }
}
