use alloy_primitives::{Address, Bytes, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionFrameType {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    Create,
    Create2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutionFrameStatus {
    Success,
    Revert,
    Halt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionFrame {
    pub(crate) frame_type: ExecutionFrameType,
    pub(crate) status: ExecutionFrameStatus,
    pub(crate) from: Address,
    pub(crate) to: Option<Address>,
    pub(crate) code_address: Option<Address>,
    pub(crate) value: U256,
    pub(crate) input: Bytes,
    pub(crate) output: Bytes,
    pub(crate) gas: u64,
    pub(crate) gas_used: u64,
    pub(crate) trace_address: Vec<u64>,
}

pub(crate) fn sort_execution_frames(frames: &mut [ExecutionFrame]) {
    frames.sort_by(|left, right| left.trace_address.cmp(&right.trace_address));
}
