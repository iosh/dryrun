use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::Address;
use conflux_provider::CoreAddress;
use conflux_simulation::core_space as simulation_core_space;
use serde::Serialize;

use super::core_space_change;

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Core Space address: {message}")]
pub(crate) struct ResponseMappingError {
    field: String,
    message: String,
}

impl ResponseMappingError {
    pub(super) fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct SimulateCoreSpaceTransactionResponse(
    std::sync::Arc<
        simulation_core::simulation::Simulation<
            simulation_core_space::CoreSpaceBlockContext,
            simulation_core_space::CoreSpaceTypedTransaction,
            simulation_core_space::CoreSpaceTransactionRequest,
            simulation_core_space::CoreSpaceExecutionOutcome,
            simulation_core_space::CoreSpaceTransactionRejection,
            Vec<core_space_change::WireChangeItem>,
            simulation_core_space::CoreSpaceChangeDerivationError,
        >,
    >,
);

impl SimulateCoreSpaceTransactionResponse {
    pub(crate) fn try_from_simulation(
        simulation: simulation_core_space::CoreSpaceSimulation,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        simulation
            .try_map_changes(|changes| {
                core_space_change::try_map_changes(changes.into_items(), network)
            })
            .map(|simulation| Self(std::sync::Arc::new(simulation)))
    }
}

pub(super) fn map_core_address(
    address: CoreAddress,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    if address.network() != provider_network(network) {
        return Err(ResponseMappingError {
            field,
            message: format!(
                "address uses network {}, expected {network}",
                address.network()
            ),
        });
    }

    map_core_space_address(Address::from_slice(&address.bytes()), network, field)
}

fn provider_network(network: Network) -> conflux_provider::Network {
    match network {
        Network::Main => conflux_provider::Network::Main,
        Network::Test => conflux_provider::Network::Test,
        Network::Id(id) => conflux_provider::Network::Id(id),
    }
}

pub(super) fn map_core_space_address(
    address: Address,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    RpcAddress::try_from_h160(address, network)
        .map_err(|message| ResponseMappingError { field, message })
}
