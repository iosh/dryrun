use crate::frames::{
    ExecutionFrame, ExecutionFrameStatus, ExecutionFrameType, sort_execution_frames,
};
use alloy_primitives::{Bytes, U256};
use revm::{
    Inspector,
    context::ContextTr,
    context_interface::CreateScheme,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, InstructionResult, InterpreterTypes,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TraceType {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    Create,
    Create2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum TraceStatus {
    Success,
    Revert,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TraceItem {
    pub(crate) trace_type: TraceType,
    pub(crate) status: TraceStatus,
    pub(crate) from: alloy_primitives::Address,
    pub(crate) to: Option<alloy_primitives::Address>,
    pub(crate) code_address: Option<alloy_primitives::Address>,
    pub(crate) value: U256,
    pub(crate) input: Bytes,
    pub(crate) output: Bytes,
    pub(crate) gas: u64,
    pub(crate) gas_used: u64,
    pub(crate) trace_address: Vec<u64>,
}

#[derive(Debug, Clone)]
struct TraceStackFrame {
    frame: ExecutionFrame,
    children_count: u64,
}

#[derive(Debug, Default)]
pub(crate) struct TraceInspector {
    call_stack: Vec<TraceStackFrame>,
    frames: Vec<ExecutionFrame>,
}

impl TraceInspector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn into_frames(mut self) -> Vec<ExecutionFrame> {
        sort_execution_frames(&mut self.frames);
        self.frames
    }

    fn next_trace_address(&mut self) -> Vec<u64> {
        if let Some(parent) = self.call_stack.last_mut() {
            let mut trace_address = parent.frame.trace_address.clone();
            trace_address.push(parent.children_count);
            parent.children_count += 1;
            trace_address
        } else {
            Vec::new()
        }
    }
}

#[allow(dead_code)]
pub(crate) fn trace_items_from_frames(mut frames: Vec<ExecutionFrame>) -> Vec<TraceItem> {
    sort_execution_frames(&mut frames);

    frames.into_iter().map(trace_item_from_frame).collect()
}

impl<CTX, INTR> Inspector<CTX, INTR> for TraceInspector
where
    CTX: ContextTr,
    INTR: InterpreterTypes,
{
    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        let frame = build_call_frame(
            inputs,
            inputs.input.bytes(context),
            self.next_trace_address(),
        );

        self.call_stack.push(TraceStackFrame {
            frame,
            children_count: 0,
        });

        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, outcome: &mut CallOutcome) {
        if let Some(mut frame) = self.call_stack.pop() {
            finalize_call_frame(&mut frame.frame, outcome);
            self.frames.push(frame.frame);
        }
    }

    fn create(&mut self, _context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        let frame = build_create_frame(inputs, self.next_trace_address());

        self.call_stack.push(TraceStackFrame {
            frame,
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
            finalize_create_frame(&mut frame.frame, outcome);
            self.frames.push(frame.frame);
        }
    }
}

fn map_call_scheme_to_frame_type(scheme: revm::interpreter::CallScheme) -> ExecutionFrameType {
    match scheme {
        revm::interpreter::CallScheme::Call => ExecutionFrameType::Call,
        revm::interpreter::CallScheme::CallCode => ExecutionFrameType::CallCode,
        revm::interpreter::CallScheme::DelegateCall => ExecutionFrameType::DelegateCall,
        revm::interpreter::CallScheme::StaticCall => ExecutionFrameType::StaticCall,
    }
}
fn map_execution_frame_type_to_trace_type(frame_type: ExecutionFrameType) -> TraceType {
    match frame_type {
        ExecutionFrameType::Call => TraceType::Call,
        ExecutionFrameType::CallCode => TraceType::CallCode,
        ExecutionFrameType::DelegateCall => TraceType::DelegateCall,
        ExecutionFrameType::StaticCall => TraceType::StaticCall,
        ExecutionFrameType::Create => TraceType::Create,
        ExecutionFrameType::Create2 => TraceType::Create2,
    }
}

fn map_create_scheme_to_frame_type(scheme: CreateScheme) -> ExecutionFrameType {
    match scheme {
        CreateScheme::Create | CreateScheme::Custom { .. } => ExecutionFrameType::Create,
        CreateScheme::Create2 { .. } => ExecutionFrameType::Create2,
    }
}

fn build_call_frame(inputs: &CallInputs, input: Bytes, trace_address: Vec<u64>) -> ExecutionFrame {
    ExecutionFrame {
        frame_type: map_call_scheme_to_frame_type(inputs.scheme),
        status: ExecutionFrameStatus::Success,
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

fn build_create_frame(inputs: &CreateInputs, trace_address: Vec<u64>) -> ExecutionFrame {
    ExecutionFrame {
        frame_type: map_create_scheme_to_frame_type(inputs.scheme()),
        status: ExecutionFrameStatus::Success,
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

fn finalize_call_frame(frame: &mut ExecutionFrame, outcome: &CallOutcome) {
    frame.status = map_instruction_result_to_frame_status(outcome.instruction_result());
    frame.gas_used = outcome.gas().spent();
    frame.output = outcome.output().clone();
}
fn finalize_create_frame(frame: &mut ExecutionFrame, outcome: &CreateOutcome) {
    frame.status = map_instruction_result_to_frame_status(outcome.instruction_result());
    frame.to = outcome.address;
    frame.gas_used = outcome.gas().spent();
    frame.output = outcome.output().clone();
}
fn map_instruction_result_to_frame_status(result: &InstructionResult) -> ExecutionFrameStatus {
    if result.is_ok() {
        ExecutionFrameStatus::Success
    } else if result.is_revert() {
        ExecutionFrameStatus::Revert
    } else {
        ExecutionFrameStatus::Halt
    }
}

fn map_execution_frame_status_to_trace_status(status: ExecutionFrameStatus) -> TraceStatus {
    match status {
        ExecutionFrameStatus::Success => TraceStatus::Success,
        ExecutionFrameStatus::Revert => TraceStatus::Revert,
        ExecutionFrameStatus::Halt => TraceStatus::Halt,
    }
}

fn trace_item_from_frame(frame: ExecutionFrame) -> TraceItem {
    TraceItem {
        trace_type: map_execution_frame_type_to_trace_type(frame.frame_type),
        status: map_execution_frame_status_to_trace_status(frame.status),
        from: frame.from,
        to: frame.to,
        code_address: frame.code_address,
        value: frame.value,
        input: frame.input,
        output: frame.output,
        gas: frame.gas,
        gas_used: frame.gas_used,
        trace_address: frame.trace_address,
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
        interpreter::{CallInput, CallScheme, CallValue, InstructionResult},
    };

    fn address(value: &str) -> Address {
        Address::from_str(value).expect("address")
    }

    fn sample_execution_frame(
        frame_type: ExecutionFrameType,
        status: ExecutionFrameStatus,
        trace_address: Vec<u64>,
    ) -> ExecutionFrame {
        ExecutionFrame {
            frame_type,
            status,
            from: address("0x1111111111111111111111111111111111111111"),
            to: Some(address("0x2222222222222222222222222222222222222222")),
            code_address: Some(address("0x3333333333333333333333333333333333333333")),
            value: U256::from(7_u64),
            input: Bytes::from_static(&[0xaa, 0xbb]),
            output: Bytes::from_static(&[0xcc]),
            gas: 50_000,
            gas_used: 21_000,
            trace_address,
        }
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
    fn delegatecall_frame_keeps_runtime_target_and_records_code_address() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let target = address("0x2222222222222222222222222222222222222222");
        let code = address("0x3333333333333333333333333333333333333333");
        let frame = build_call_frame(
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

        assert_eq!(frame.frame_type, ExecutionFrameType::DelegateCall);
        assert_eq!(frame.from, caller);
        assert_eq!(frame.to, Some(target));
        assert_eq!(frame.code_address, Some(code));
        assert_eq!(frame.value, U256::ZERO);
        assert_eq!(frame.trace_address, vec![0, 1]);
    }

    #[test]
    fn callcode_frame_uses_self_target_and_external_code_address() {
        let current = address("0x2222222222222222222222222222222222222222");
        let library = address("0x3333333333333333333333333333333333333333");
        let frame = build_call_frame(
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

        assert_eq!(frame.frame_type, ExecutionFrameType::CallCode);
        assert_eq!(frame.from, current);
        assert_eq!(frame.to, Some(current));
        assert_eq!(frame.code_address, Some(library));
        assert_eq!(frame.value, U256::from(9_u64));
    }

    #[test]
    fn custom_create_scheme_maps_to_create_not_create2() {
        assert_eq!(
            map_create_scheme_to_frame_type(CreateScheme::Custom {
                address: address("0x4444444444444444444444444444444444444444"),
            }),
            ExecutionFrameType::Create
        );
    }

    #[test]
    fn create_frame_has_no_code_address_before_completion() {
        let inputs = CreateInputs::new(
            address("0x1111111111111111111111111111111111111111"),
            CreateScheme::Create2 {
                salt: U256::from(5_u64),
            },
            U256::from(11_u64),
            Bytes::from_static(&[0x60, 0x00]),
            80_000,
        );
        let frame = build_create_frame(&inputs, vec![2]);

        assert_eq!(frame.frame_type, ExecutionFrameType::Create2);
        assert_eq!(frame.to, None);
        assert_eq!(frame.code_address, None);
        assert_eq!(frame.value, U256::from(11_u64));
        assert_eq!(frame.trace_address, vec![2]);
    }

    #[test]
    fn instruction_result_maps_into_execution_frame_status_categories() {
        assert_eq!(
            map_instruction_result_to_frame_status(&InstructionResult::Return),
            ExecutionFrameStatus::Success
        );
        assert_eq!(
            map_instruction_result_to_frame_status(&InstructionResult::Revert),
            ExecutionFrameStatus::Revert
        );
        assert_eq!(
            map_instruction_result_to_frame_status(&InstructionResult::OutOfGas),
            ExecutionFrameStatus::Halt
        );
    }

    #[test]
    fn execution_frame_maps_into_trace_item() {
        let trace = trace_item_from_frame(sample_execution_frame(
            ExecutionFrameType::Create2,
            ExecutionFrameStatus::Revert,
            vec![1, 2],
        ));

        assert_eq!(trace.trace_type, TraceType::Create2);
        assert_eq!(trace.status, TraceStatus::Revert);
        assert_eq!(
            trace.from,
            address("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            trace.to,
            Some(address("0x2222222222222222222222222222222222222222"))
        );
        assert_eq!(
            trace.code_address,
            Some(address("0x3333333333333333333333333333333333333333"))
        );
        assert_eq!(trace.value, U256::from(7_u64));
        assert_eq!(trace.input, Bytes::from_static(&[0xaa, 0xbb]));
        assert_eq!(trace.output, Bytes::from_static(&[0xcc]));
        assert_eq!(trace.gas, 50_000);
        assert_eq!(trace.gas_used, 21_000);
        assert_eq!(trace.trace_address, vec![1, 2]);
    }

    #[test]
    fn trace_items_from_frames_sorts_by_trace_address() {
        let trace = trace_items_from_frames(vec![
            sample_execution_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                vec![1],
            ),
            sample_execution_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                Vec::new(),
            ),
            sample_execution_frame(
                ExecutionFrameType::Call,
                ExecutionFrameStatus::Success,
                vec![0, 0],
            ),
        ]);

        assert_eq!(trace.len(), 3);
        assert_eq!(trace[0].trace_address, Vec::<u64>::new());
        assert_eq!(trace[1].trace_address, vec![0, 0]);
        assert_eq!(trace[2].trace_address, vec![1]);
    }
}
