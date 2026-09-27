use std::sync::Arc;

use alloy::{
    network::Ethereum,
    primitives::U256,
    providers::{DynProvider, Provider},
};
use conflux_provider::{ConfluxProvider, CoreStatus, Network};

use crate::{
    ConfluxCoreStatusIdentityField, ConfluxEndpointIdentity, ConfluxInitializationError,
    chain_spec::ConfluxChainSpec, state::ConfluxSimulationProvider,
};

/// A reusable Conflux mainnet simulation backend verified against its RPC endpoints.
///
/// Cloning this value shares its immutable chain specification and provider clients.
#[derive(Clone)]
pub struct ConfluxSimulationBackend {
    inner: Arc<ConfluxSimulationBackendInner>,
}

struct ConfluxSimulationBackendInner {
    chain_spec: ConfluxChainSpec,
    provider: ConfluxSimulationProvider,
}

impl ConfluxSimulationBackend {
    /// Creates a Conflux mainnet backend after validating both endpoint identities.
    ///
    /// The eSpace provider is type-erased so caller-installed provider layers can be
    /// retained without making the backend generic. This checks the Core Space chain
    /// id, Core-reported eSpace chain id, network id, and the eSpace endpoint chain id.
    /// No backend is returned if either request fails or any identity value differs.
    ///
    /// Authentication, retry, and request-timeout policies remain the caller's
    /// responsibility. Identity validation is performed once during construction.
    pub async fn mainnet(
        espace_provider: DynProvider<Ethereum>,
        core_space_provider: ConfluxProvider,
    ) -> Result<Self, ConfluxInitializationError> {
        let chain_spec = ConfluxChainSpec::mainnet();
        let core_status_request = async {
            core_space_provider
                .cfx_get_status()
                .await
                .map_err(|source| ConfluxInitializationError::CoreStatusRequest { source })
        };
        let espace_chain_id_request = async {
            espace_provider
                .get_chain_id()
                .await
                .map_err(|source| ConfluxInitializationError::EspaceChainIdRequest { source })
        };
        let (core_status, espace_chain_id) =
            tokio::try_join!(core_status_request, espace_chain_id_request)?;

        validate_mainnet_identity(&chain_spec, &core_status, espace_chain_id)?;

        let provider = ConfluxSimulationProvider::new(
            espace_provider,
            core_space_provider,
            chain_spec.core_space_address_network(),
        );

        Ok(Self {
            inner: Arc::new(ConfluxSimulationBackendInner {
                chain_spec,
                provider,
            }),
        })
    }

    /// Returns the network used to encode and validate Core Space addresses.
    pub fn core_space_address_network(&self) -> Network {
        self.chain_spec().core_space_address_network()
    }

    pub(crate) fn chain_spec(&self) -> &ConfluxChainSpec {
        &self.inner.chain_spec
    }

    pub(crate) fn provider(&self) -> &ConfluxSimulationProvider {
        &self.inner.provider
    }
}

fn validate_mainnet_identity(
    chain_spec: &ConfluxChainSpec,
    core_status: &CoreStatus,
    espace_chain_id: u64,
) -> Result<(), ConfluxInitializationError> {
    let expected_espace_chain_id = u64::from(chain_spec.espace_chain_id());
    let expected = ConfluxEndpointIdentity::new(
        u64::from(chain_spec.core_space_chain_id()),
        expected_espace_chain_id,
        chain_spec.network_id(),
        expected_espace_chain_id,
    );
    let actual = ConfluxEndpointIdentity::new(
        core_status_identity_value(
            ConfluxCoreStatusIdentityField::ChainId,
            core_status.chain_id,
        )?,
        core_status_identity_value(
            ConfluxCoreStatusIdentityField::EthereumSpaceChainId,
            core_status.ethereum_space_chain_id,
        )?,
        core_status_identity_value(
            ConfluxCoreStatusIdentityField::NetworkId,
            core_status.network_id,
        )?,
        espace_chain_id,
    );

    if actual != expected {
        return Err(ConfluxInitializationError::EndpointIdentityMismatch { expected, actual });
    }

    Ok(())
}

fn core_status_identity_value(
    field: ConfluxCoreStatusIdentityField,
    actual: U256,
) -> Result<u64, ConfluxInitializationError> {
    u64::try_from(actual).map_err(|_| {
        ConfluxInitializationError::CoreStatusIdentityValueOutOfRange { field, actual }
    })
}
