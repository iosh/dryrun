use alloy::primitives::Address;
use revm::{
    Inspector,
    context::{ContextTr, JournalEntry},
    inspector::JournalExt,
    interpreter::{CallInputs, CallOutcome, CreateInputs, CreateOutcome, InterpreterTypes},
};

#[derive(Debug, Default)]
pub(crate) struct EvmExecutionObserver {
    applied_authorization_accounts: Option<Vec<Address>>,
}

impl EvmExecutionObserver {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn take_applied_authorization_accounts(&mut self) -> Vec<Address> {
        self.applied_authorization_accounts
            .take()
            .unwrap_or_default()
    }

    fn observe_transaction_start<CTX>(&mut self, context: &CTX)
    where
        CTX: ContextTr,
        CTX::Journal: JournalExt,
    {
        if self.applied_authorization_accounts.is_some() {
            return;
        }

        let mut accounts = context
            .journal()
            .journal()
            .iter()
            .filter_map(|entry| match entry {
                JournalEntry::CodeChange { address } => Some(*address),
                _ => None,
            })
            .collect::<Vec<_>>();
        accounts.sort_unstable();
        accounts.dedup();
        self.applied_authorization_accounts = Some(accounts);
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for EvmExecutionObserver
where
    CTX: ContextTr,
    CTX::Journal: JournalExt,
    INTR: InterpreterTypes,
{
    fn call(&mut self, context: &mut CTX, _inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.observe_transaction_start(context);
        None
    }

    fn create(&mut self, context: &mut CTX, _inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.observe_transaction_start(context);
        None
    }
}
