use alloy_primitives::{B256, Bytes, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Execution<Context, Details, Failure> {
    pub chain_id: u64,
    pub context: Context,
    pub gas_limit: u64,
    pub outcome: ExecutionOutcome<Details, Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome<Details, Failure> {
    Success(Details),
    Failed { details: Details, failure: Failure },
    NotExecuted(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedDetails<BurntFee = U256> {
    pub gas_used: u64,
    pub gas_charged: u64,
    pub fee: U256,
    pub burnt_fee: BurntFee,
    pub output: Bytes,
}
