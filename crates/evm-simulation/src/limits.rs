#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulationLimits {
    pub max_occurrence_checkpoints: usize,
    pub max_retained_state_entries: usize,
    pub max_state_reads: usize,
    pub max_read_calls: usize,
    pub read_call_gas_limit: u64,
    pub max_read_call_output_bytes: usize,
}

impl EvmSimulationLimits {
    pub const fn new(
        max_occurrence_checkpoints: usize,
        max_retained_state_entries: usize,
        max_state_reads: usize,
        max_read_calls: usize,
        read_call_gas_limit: u64,
        max_read_call_output_bytes: usize,
    ) -> Self {
        Self {
            max_occurrence_checkpoints,
            max_retained_state_entries,
            max_state_reads,
            max_read_calls,
            read_call_gas_limit,
            max_read_call_output_bytes,
        }
    }
}
