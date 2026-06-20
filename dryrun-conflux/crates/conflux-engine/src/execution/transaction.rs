use cfx_types::{Address, AddressSpaceUtil};
use primitives::{SignedTransaction, transaction::EthereumTransaction};

#[derive(Debug, Clone)]
pub struct EspaceTransactionInput {
    pub tx: EthereumTransaction,
    pub sender: Address,
}

pub fn signed_transaction_for_dryrun(input: EspaceTransactionInput) -> SignedTransaction {
    // Dryrun input has an explicit sender but no real signature. Conflux
    // executor still requires a SignedTransaction, so use upstream's RPC fake
    // signature path to bind the eSpace sender.
    input.tx.fake_sign_rpc(input.sender.with_evm_space())
}
