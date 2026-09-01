#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvmSimulationLimits {
    pub max_occurrence_checkpoints: Option<usize>,
    pub max_retained_state_entries: Option<usize>,
    pub max_state_reads: Option<usize>,
    pub max_read_calls: Option<usize>,
    pub read_call_gas_limit: Option<u64>,
    pub max_read_call_output_bytes: Option<usize>,
}
