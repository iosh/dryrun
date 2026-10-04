use crate::{Error, address::VmAddress, primitive::*};
use alloy::primitives::{B256, LogData, U256};
use cfx_executor::{
    machine::Machine,
    observer::{
        AsTracer, CallTracer, CheckpointTracer, DrainTrace, InternalTransferTracer, OpcodeTracer,
        SetAuthTracer, StorageTracer, TracerTrait,
    },
    stack::FrameResult,
    state::State,
};
use cfx_types::{AddressWithSpace, Space};
use cfx_vm_types::{ActionParams, ActionValue, CallType, CreateType, Env, Spec};
use conflux_provider::Network;
use simulation_core::{AccountDiff, CallFrame, CallScheme, Diff, ExecutionTrace, Log};
use std::collections::BTreeMap;

pub(crate) struct Tracer<'a, A> {
    frames: Vec<CallFrame<A>>,
    stack: Vec<Option<usize>>,
    machine: &'a Machine,
    number: u64,
    epoch: u64,
    network: Network,
    spec: &'a Spec,
}

impl<'a, A: VmAddress> Tracer<'a, A> {
    pub fn new(machine: &'a Machine, env: &Env, spec: &'a Spec, network: Network) -> Self {
        Self {
            frames: Vec::new(),
            stack: Vec::new(),
            machine,
            number: env.number,
            epoch: env.epoch_height,
            network,
            spec,
        }
    }

    /// Combines this execution's callbacks, retained logs and state differences.
    pub fn into_trace(
        self,
        before: &State,
        after: &State,
        logs: Vec<primitives::LogEntry>,
    ) -> Result<ExecutionTrace<A>, Error> {
        let accounts = account_diffs(before, after, self.network)?;
        let logs = logs
            .into_iter()
            .filter(|log| A::accepts(log.space))
            .map(|log| Log {
                address: A::from_vm(
                    AddressWithSpace {
                        address: log.address,
                        space: log.space,
                    },
                    self.network,
                ),
                data: LogData::new_unchecked(
                    log.topics.into_iter().map(b256_from_cfx).collect(),
                    log.data.into(),
                ),
            })
            .collect();
        Ok(ExecutionTrace {
            calls: self.frames,
            logs,
            accounts,
        })
    }

    fn enter(&mut self, params: &ActionParams, create: bool) {
        let runs_code = A::accepts(params.space)
            && (params.code.as_ref().is_some_and(|code| !code.is_empty())
                || (params.space == Space::Native
                    && self
                        .machine
                        .internal_contracts()
                        .contract(
                            &AddressWithSpace {
                                address: params.code_address,
                                space: params.space,
                            },
                            self.spec,
                        )
                        .is_some()))
            && (create
                || self
                    .machine
                    .builtin(
                        &AddressWithSpace {
                            address: params.code_address,
                            space: params.space,
                        },
                        self.number,
                        self.epoch,
                    )
                    .is_none());
        let index = runs_code.then(|| {
            let index = self.frames.len();
            let scheme = if create {
                if params.create_type == CreateType::CREATE2 {
                    CallScheme::Create2
                } else {
                    CallScheme::Create
                }
            } else {
                match params.call_type {
                    CallType::Call | CallType::None => CallScheme::Call,
                    CallType::CallCode => CallScheme::CallCode,
                    CallType::DelegateCall => CallScheme::DelegateCall,
                    CallType::StaticCall => CallScheme::StaticCall,
                }
            };
            self.frames.push(CallFrame {
                parent: self.stack.iter().rev().find_map(|index| *index),
                scheme,
                from: A::from_vm(
                    AddressWithSpace {
                        address: params.sender,
                        space: params.space,
                    },
                    self.network,
                ),
                to: A::from_vm(
                    AddressWithSpace {
                        address: params.address,
                        space: params.space,
                    },
                    self.network,
                ),
                code_address: A::from_vm(
                    AddressWithSpace {
                        address: params.code_address,
                        space: params.space,
                    },
                    self.network,
                ),
                value: match params.value {
                    ActionValue::Transfer(v) => u256_from_cfx(v),
                    ActionValue::Apparent(_) => U256::ZERO,
                },
                input: if create {
                    params
                        .code
                        .as_ref()
                        .map(|code| code.as_ref().clone())
                        .unwrap_or_default()
                        .into()
                } else {
                    params.data.clone().unwrap_or_default().into()
                },
                success: false,
            });
            index
        });
        // Even omitted builtin/empty frames have matching return callbacks.
        self.stack.push(index);
    }

    fn leave(&mut self, result: &FrameResult) {
        if let Some(Some(index)) = self.stack.pop() {
            self.frames[index].success = result.as_ref().is_ok_and(|result| result.apply_state);
        }
    }
}

impl<A: VmAddress> CallTracer for Tracer<'_, A> {
    fn record_call(&mut self, params: &ActionParams) {
        self.enter(params, false);
    }
    fn record_create(&mut self, params: &ActionParams) {
        self.enter(params, true);
    }
    fn record_call_result(&mut self, result: &FrameResult) {
        self.leave(result);
    }
    fn record_create_result(&mut self, result: &FrameResult) {
        self.leave(result);
    }
}
impl<A: VmAddress> CheckpointTracer for Tracer<'_, A> {}
impl<A: VmAddress> InternalTransferTracer for Tracer<'_, A> {}
impl<A: VmAddress> StorageTracer for Tracer<'_, A> {}
impl<A: VmAddress> OpcodeTracer for Tracer<'_, A> {}
impl<A: VmAddress> SetAuthTracer for Tracer<'_, A> {}
impl<A: VmAddress> AsTracer for &mut Tracer<'_, A> {
    fn as_tracer<'a>(&'a mut self) -> Box<dyn TracerTrait + 'a> {
        Box::new(&mut **self)
    }
}
impl<A: VmAddress> DrainTrace for &mut Tracer<'_, A> {
    fn drain_trace(self, _map: &mut typemap::ShareDebugMap) {}
}

fn account_diffs<A: VmAddress>(
    before: &State,
    after: &State,
    network: Network,
) -> Result<BTreeMap<A, AccountDiff>, Error> {
    let mut accounts = BTreeMap::new();
    for (address, entry) in &after.committed_cache {
        if !A::accepts(address.space) || !entry.is_dirty() {
            continue;
        }
        let mut storage = BTreeMap::new();
        if let Some(account) = entry.account() {
            for key in account.modified_storage_keys() {
                let before_value = B256::from(u256_from_cfx(before.storage_at(address, &key)?));
                let after_value = B256::from(u256_from_cfx(after.storage_at(address, &key)?));
                if before_value != after_value {
                    storage.insert(
                        key.into(),
                        Diff {
                            before: before_value,
                            after: after_value,
                        },
                    );
                }
            }
        }
        let delegation = |state: &State| -> Result<_, Error> {
            if address.space == cfx_types::Space::Native {
                return Ok(None);
            }
            Ok(state
                .code(address)?
                .and_then(|code| primitives::transaction::extract_7702_payload(&code))
                .map(address_from_cfx))
        };
        accounts.insert(
            A::from_vm(*address, network),
            AccountDiff {
                balance: Diff {
                    before: u256_from_cfx(before.balance(address)?),
                    after: u256_from_cfx(after.balance(address)?),
                },
                nonce: Diff {
                    before: u256_from_cfx(before.nonce(address)?),
                    after: u256_from_cfx(after.nonce(address)?),
                },
                code_hash: Diff {
                    before: b256_from_cfx(before.code_hash(address)?),
                    after: b256_from_cfx(after.code_hash(address)?),
                },
                delegation: Diff {
                    before: delegation(before)?,
                    after: delegation(after)?,
                },
                storage,
            },
        );
    }
    Ok(accounts)
}
