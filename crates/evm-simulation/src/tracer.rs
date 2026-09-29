use alloy::primitives::Address;
use revm::{
    Inspector,
    context_interface::{ContextTr, JournalTr},
    interpreter::{
        CallInputs, CallOutcome, CallScheme as RevmCallScheme, CreateInputs, CreateOutcome,
        CreateScheme,
    },
    state::EvmState,
};
use simulation_core::{CallFrame, CallScheme};

/// Records the frames that run code.
#[derive(Debug, Default)]
pub(crate) struct CallTracer {
    pub(crate) calls: Vec<CallFrame<Address>>,
    /// Index in `calls` of each frame being executed; `None` for frames that
    /// are not recorded.
    stack: Vec<Option<usize>>,
}

impl CallTracer {
    fn enter(&mut self, frame: CallFrame<Address>) {
        self.stack.push(Some(self.calls.len()));
        self.calls.push(frame);
    }

    fn parent(&self) -> Option<usize> {
        self.stack.last().copied().flatten()
    }
}

impl<CTX: ContextTr<Journal: JournalTr<State = EvmState>>> Inspector<CTX> for CallTracer {
    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        // Accounts without code and precompiles run no code.
        if inputs.known_bytecode.1.is_empty() {
            self.stack.push(None);
            return None;
        }
        self.enter(CallFrame {
            parent: self.parent(),
            scheme: match inputs.scheme {
                RevmCallScheme::Call => CallScheme::Call,
                RevmCallScheme::CallCode => CallScheme::CallCode,
                RevmCallScheme::DelegateCall => CallScheme::DelegateCall,
                RevmCallScheme::StaticCall => CallScheme::StaticCall,
            },
            from: inputs.caller,
            to: inputs.target_address,
            code_address: code_address(context, inputs.bytecode_address),
            value: inputs.transfer_value().unwrap_or_default(),
            input: inputs.input.bytes(context),
            success: false,
        });
        None
    }

    fn call_end(&mut self, _: &mut CTX, _: &CallInputs, outcome: &mut CallOutcome) {
        if let Some(Some(index)) = self.stack.pop() {
            self.calls[index].success = outcome.result.is_ok();
        }
    }

    fn create(&mut self, _: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.enter(CallFrame {
            parent: self.parent(),
            scheme: match inputs.scheme() {
                CreateScheme::Create2 { .. } => CallScheme::Create2,
                CreateScheme::Create | CreateScheme::Custom { .. } => CallScheme::Create,
            },
            from: inputs.caller(),
            // Known once the frame starts; set in `create_end`.
            to: Address::ZERO,
            code_address: Address::ZERO,
            value: inputs.value(),
            input: inputs.init_code().clone(),
            success: false,
        });
        None
    }

    fn create_end(&mut self, _: &mut CTX, _: &CreateInputs, outcome: &mut CreateOutcome) {
        let Some(Some(index)) = self.stack.pop() else {
            return;
        };
        match outcome.address {
            Some(address) => {
                let frame = &mut self.calls[index];
                frame.to = address;
                frame.code_address = address;
                frame.success = outcome.result.is_ok();
            }
            // The creation failed before its frame started, so it ran no
            // code and has no children.
            None => {
                self.calls.truncate(index);
            }
        }
    }
}

/// The account whose code a call to `address` runs. A call to an EIP-7702
/// delegated account runs its delegate's code, while revm keeps the delegated
/// account as the call's bytecode address.
fn code_address<CTX: ContextTr<Journal: JournalTr<State = EvmState>>>(
    context: &CTX,
    address: Address,
) -> Address {
    // The call has already loaded the account with its code.
    context
        .journal()
        .evm_state()
        .get(&address)
        .and_then(|account| account.info.code.as_ref()?.eip7702_address())
        .unwrap_or(address)
}
