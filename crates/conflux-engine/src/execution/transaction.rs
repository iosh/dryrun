use cfx_types::{Address, AddressSpaceUtil};
use primitives::{
    SignedTransaction,
    transaction::{EthereumTransaction, TypedNativeTransaction},
};

#[derive(Debug, Clone)]
pub enum DryRunTransactionInput {
    Espace(EspaceTransactionInput),
    Native(NativeTransactionInput),
}

#[derive(Debug, Clone)]
pub struct EspaceTransactionInput {
    pub tx: EthereumTransaction,
    pub sender: Address,
}

#[derive(Debug, Clone)]
pub struct NativeTransactionInput {
    pub tx: TypedNativeTransaction,
    pub sender: Address,
}

pub fn signed_transaction_for_dryrun(input: DryRunTransactionInput) -> SignedTransaction {
    match input {
        DryRunTransactionInput::Espace(input) => {
            // Dryrun input has an explicit sender but no real signature.
            // Conflux executor still requires a SignedTransaction, so use
            // upstream's RPC fake signature path to bind the eSpace sender.
            input.tx.fake_sign_rpc(input.sender.with_evm_space())
        }
        DryRunTransactionInput::Native(input) => {
            input.tx.fake_sign_rpc(input.sender.with_native_space())
        }
    }
}
