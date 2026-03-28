use crate::{TraceItem, TraceType};
use alloy_primitives::{Bytes, U256};
use revm::{
    Inspector,
    context::ContextTr,
    context_interface::CreateScheme,
    interpreter::{CallInputs, CallOutcome, CreateInputs, CreateOutcome, InterpreterTypes},
};

#[derive(Debug, Clone)]
struct TraceStackFrame {
    trace: TraceItem,
    children_count: u64,
}

#[derive(Debug, Default)]
pub(crate) struct TraceInspector {
    call_stack: Vec<TraceStackFrame>,
    traces: Vec<TraceItem>,
}

impl TraceInspector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn into_traces(mut self) -> Vec<TraceItem> {
        self.traces
            .sort_by(|left, right| left.trace_address.cmp(&right.trace_address));
        self.traces
    }

    fn next_trace_address(&mut self) -> Vec<u64> {
        if let Some(parent) = self.call_stack.last_mut() {
            let mut trace_address = parent.trace.trace_address.clone();
            trace_address.push(parent.children_count);
            parent.children_count += 1;
            trace_address
        } else {
            Vec::new()
        }
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for TraceInspector
where
    CTX: ContextTr,
    INTR: InterpreterTypes,
{
    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let trace = build_call_trace(
            inputs,
            inputs.input.bytes(context),
            self.next_trace_address(),
        );

        self.call_stack.push(TraceStackFrame {
            trace,
            children_count: 0,
        });

        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, outcome: &mut CallOutcome) {
        if let Some(mut frame) = self.call_stack.pop() {
            frame.trace.gas_used = outcome.gas().spent();
            frame.trace.output = outcome.output().clone();
            self.traces.push(frame.trace);
        }
    }

    fn create(&mut self, _context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        let trace = build_create_trace(inputs, self.next_trace_address());

        self.call_stack.push(TraceStackFrame {
            trace,
            children_count: 0,
        });

        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        if let Some(mut frame) = self.call_stack.pop() {
            frame.trace.to = outcome.address;
            frame.trace.gas_used = outcome.gas().spent();
            frame.trace.output = outcome.output().clone();
            self.traces.push(frame.trace);
        }
    }
}

fn map_call_scheme_to_trace_type(scheme: revm::interpreter::CallScheme) -> TraceType {
    match scheme {
        revm::interpreter::CallScheme::Call => TraceType::Call,
        revm::interpreter::CallScheme::CallCode => TraceType::CallCode,
        revm::interpreter::CallScheme::DelegateCall => TraceType::DelegateCall,
        revm::interpreter::CallScheme::StaticCall => TraceType::StaticCall,
    }
}

fn map_create_scheme_to_trace_type(scheme: CreateScheme) -> TraceType {
    match scheme {
        CreateScheme::Create | CreateScheme::Custom { .. } => TraceType::Create,
        CreateScheme::Create2 { .. } => TraceType::Create2,
    }
}

fn build_call_trace(inputs: &CallInputs, input: Bytes, trace_address: Vec<u64>) -> TraceItem {
    TraceItem {
        trace_type: map_call_scheme_to_trace_type(inputs.scheme),
        from: inputs.caller,
        to: Some(inputs.target_address),
        code_address: Some(inputs.bytecode_address),
        value: inputs.transfer_value().unwrap_or(U256::ZERO),
        input,
        output: Default::default(),
        gas: inputs.gas_limit,
        gas_used: 0,
        trace_address,
    }
}

fn build_create_trace(inputs: &CreateInputs, trace_address: Vec<u64>) -> TraceItem {
    TraceItem {
        trace_type: map_create_scheme_to_trace_type(inputs.scheme()),
        from: inputs.caller(),
        to: None,
        code_address: None,
        value: inputs.value(),
        input: inputs.init_code().clone(),
        output: Default::default(),
        gas: inputs.gas_limit(),
        gas_used: 0,
        trace_address,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;
    use std::str::FromStr;

    use alloy_primitives::{Address, B256};
    use revm::{
        bytecode::Bytecode,
        interpreter::{CallInput, CallScheme, CallValue},
    };

    fn address(value: &str) -> Address {
        Address::from_str(value).expect("address")
    }

    fn build_call_inputs(
        scheme: CallScheme,
        caller: Address,
        target_address: Address,
        bytecode_address: Address,
        value: CallValue,
    ) -> CallInputs {
        CallInputs {
            input: CallInput::Bytes(Bytes::from_static(&[0x12, 0x34])),
            return_memory_offset: Range::default(),
            gas_limit: 50_000,
            bytecode_address,
            known_bytecode: Some((B256::ZERO, Bytecode::default())),
            target_address,
            caller,
            value,
            scheme,
            is_static: false,
        }
    }

    #[test]
    fn delegatecall_trace_keeps_runtime_target_and_records_code_address() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let target = address("0x2222222222222222222222222222222222222222");
        let code = address("0x3333333333333333333333333333333333333333");
        let trace = build_call_trace(
            &build_call_inputs(
                CallScheme::DelegateCall,
                caller,
                target,
                code,
                CallValue::Apparent(U256::from(7_u64)),
            ),
            Bytes::from_static(&[0x12, 0x34]),
            vec![0, 1],
        );

        assert_eq!(trace.trace_type, TraceType::DelegateCall);
        assert_eq!(trace.from, caller);
        assert_eq!(trace.to, Some(target));
        assert_eq!(trace.code_address, Some(code));
        assert_eq!(trace.value, U256::ZERO);
        assert_eq!(trace.trace_address, vec![0, 1]);
    }

    #[test]
    fn callcode_trace_uses_self_target_and_external_code_address() {
        let current = address("0x2222222222222222222222222222222222222222");
        let library = address("0x3333333333333333333333333333333333333333");
        let trace = build_call_trace(
            &build_call_inputs(
                CallScheme::CallCode,
                current,
                current,
                library,
                CallValue::Transfer(U256::from(9_u64)),
            ),
            Bytes::from_static(&[0xab]),
            Vec::new(),
        );

        assert_eq!(trace.trace_type, TraceType::CallCode);
        assert_eq!(trace.from, current);
        assert_eq!(trace.to, Some(current));
        assert_eq!(trace.code_address, Some(library));
        assert_eq!(trace.value, U256::from(9_u64));
    }

    #[test]
    fn custom_create_scheme_maps_to_create_not_create2() {
        assert_eq!(
            map_create_scheme_to_trace_type(CreateScheme::Custom {
                address: address("0x4444444444444444444444444444444444444444"),
            }),
            TraceType::Create
        );
    }

    #[test]
    fn create_trace_has_no_code_address_before_completion() {
        let inputs = CreateInputs::new(
            address("0x1111111111111111111111111111111111111111"),
            CreateScheme::Create2 {
                salt: U256::from(5_u64),
            },
            U256::from(11_u64),
            Bytes::from_static(&[0x60, 0x00]),
            80_000,
        );
        let trace = build_create_trace(&inputs, vec![2]);

        assert_eq!(trace.trace_type, TraceType::Create2);
        assert_eq!(trace.to, None);
        assert_eq!(trace.code_address, None);
        assert_eq!(trace.value, U256::from(11_u64));
        assert_eq!(trace.trace_address, vec![2]);
    }
}
