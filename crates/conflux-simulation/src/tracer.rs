use crate::primitive::*;
use alloy::primitives::{Address, U256};
use cfx_executor::{
    machine::Machine,
    observer::{
        AsTracer, CallTracer, CheckpointTracer, DrainTrace, InternalTransferTracer, OpcodeTracer,
        SetAuthTracer, StorageTracer, TracerTrait,
    },
    stack::FrameResult,
};
use cfx_types::{AddressWithSpace, Space};
use cfx_vm_types::{ActionParams, ActionValue, CallType, CreateType};
use simulation_core::{CallFrame, CallScheme};

pub(crate) struct Calls<'a> {
    pub frames: Vec<CallFrame<Address>>,
    stack: Vec<Option<usize>>,
    machine: &'a Machine,
    number: u64,
    epoch: u64,
}

impl<'a> Calls<'a> {
    pub fn new(machine: &'a Machine, number: u64, epoch: u64) -> Self {
        Self {
            frames: Vec::new(),
            stack: Vec::new(),
            machine,
            number,
            epoch,
        }
    }

    fn enter(&mut self, params: &ActionParams, create: bool) {
        let runs_code = params.space == Space::Ethereum
            && params.code.as_ref().is_some_and(|code| !code.is_empty())
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
                from: address_from_cfx(params.sender),
                to: address_from_cfx(params.address),
                code_address: address_from_cfx(params.code_address),
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

impl CallTracer for Calls<'_> {
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
impl CheckpointTracer for Calls<'_> {}
impl InternalTransferTracer for Calls<'_> {}
impl StorageTracer for Calls<'_> {}
impl OpcodeTracer for Calls<'_> {}
impl SetAuthTracer for Calls<'_> {}
impl AsTracer for &mut Calls<'_> {
    fn as_tracer<'a>(&'a mut self) -> Box<dyn TracerTrait + 'a> {
        Box::new(&mut **self)
    }
}
impl DrainTrace for &mut Calls<'_> {
    fn drain_trace(self, _map: &mut typemap::ShareDebugMap) {}
}
