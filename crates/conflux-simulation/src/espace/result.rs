pub type EspaceSimulation = simulation_core::simulation::Simulation<
    super::EspaceBlockContext,
    super::EspaceTypedTransaction,
    super::EspaceTransactionRequest,
    super::EspaceExecutionOutcome,
    super::EspaceTransactionRejection,
    super::EspaceChangeSet,
    super::EspaceChangeDerivationError,
>;
