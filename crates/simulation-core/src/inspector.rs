use alloy::primitives::U64;
use revm::{
    Inspector,
    context::ContextTr,
    interpreter::{CallScheme, CallValue, InterpreterTypes},
    primitives::Bytes,
};
use types::{CallTraceItem, TraceActionType};

#[derive(Debug, Clone)]
struct CallStackFrame {
    trace: CallTraceItem,
    children_count: usize,
}

#[derive(Debug, Default)]
pub struct TraceInspector {
    call_stack: Vec<CallStackFrame>,
    traces: Vec<CallTraceItem>,
}

impl TraceInspector {
    pub fn new() -> Self {
        Self {
            call_stack: Vec::new(),
            traces: Vec::new(),
        }
    }

    pub fn into_traces(mut self) -> Vec<CallTraceItem> {
        self.traces
            .sort_by(|a, b| a.trace_address.cmp(&b.trace_address));
        self.traces
    }
}

impl<CTX, INSP> Inspector<CTX, INSP> for TraceInspector
where
    CTX: ContextTr,
    INSP: InterpreterTypes,
{
    fn call(
        &mut self,
        context: &mut CTX,
        inputs: &mut revm::interpreter::CallInputs,
    ) -> Option<revm::interpreter::CallOutcome> {
        let trace_address = if let Some(parent) = self.call_stack.last_mut() {
            let mut addr = parent.trace.trace_address.clone();

            addr.push(parent.children_count);
            parent.children_count += 1;
            addr
        } else {
            vec![]
        };

        let value = match inputs.value {
            CallValue::Transfer(v) | CallValue::Apparent(v) => v,
        };

        let trace = CallTraceItem {
            action_type: map_call_scheme_to_action_type(&inputs.scheme),
            from: inputs.caller,
            to: inputs.target_address,
            value,
            input: inputs.input.bytes(context),
            gas: U64::from(inputs.gas_limit),
            trace_address,
            gas_used: U64::ZERO,
            output: Bytes::new(),
            subtraces: 0,
            decode_input: None,
        };

        self.call_stack.push(CallStackFrame {
            trace,
            children_count: 0,
        });

        None
    }

    fn call_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &revm::interpreter::CallInputs,
        outcome: &mut revm::interpreter::CallOutcome,
    ) {
        if let Some(mut frame) = self.call_stack.pop() {
            let gas_used = frame.trace.gas.saturating_to::<u64>() - outcome.gas().remaining();

            frame.trace.gas_used = U64::from(gas_used);
            frame.trace.output = outcome.output().clone();
            frame.trace.subtraces = frame.children_count;

            self.traces.push(frame.trace);
        }
    }
}

fn map_call_scheme_to_action_type(scheme: &CallScheme) -> TraceActionType {
    match scheme {
        CallScheme::Call => TraceActionType::Call,
        CallScheme::CallCode => TraceActionType::Call,
        CallScheme::DelegateCall => TraceActionType::DelegateCall,
        CallScheme::StaticCall => TraceActionType::StaticCall,
    }
}
