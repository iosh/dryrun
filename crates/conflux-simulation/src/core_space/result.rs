pub type CoreSpaceChanges =
    simulation_core::simulation::Changes<super::CoreSpaceChangeSet, super::CoreSpaceAnalysisError>;

pub type CoreSpaceSimulation = simulation_core::simulation::Simulation<
    super::CoreSpaceBlockContext,
    super::CoreSpaceTypedTransaction,
    super::CoreSpaceTransactionRequest,
    super::CoreSpaceExecutionOutcome,
    super::CoreSpaceTransactionRejection,
    super::CoreSpaceChangeSet,
    super::CoreSpaceAnalysisError,
>;
