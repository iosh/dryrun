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

#[cfg(test)]
mod tests {
    use alloy::{
        network::Ethereum,
        primitives::{B256, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };
    use conflux_provider::{ConfluxProvider, ConfluxProviderError, CoreStatus, Network};

    use super::{ConfluxSimulationBackend, validate_mainnet_identity};
    use crate::{
        ConfluxCoreStatusIdentityField, ConfluxEndpointIdentity, ConfluxInitializationError,
        chain_spec::ConfluxChainSpec,
    };

    #[tokio::test]
    async fn creates_backend_for_matching_mainnet_identity() {
        let backend = initialize(mainnet_status(1029, 1030, 1029), 1030)
            .await
            .expect("matching mainnet identity should initialize");

        assert_eq!(backend.core_space_address_network(), Network::Main);
        assert_eq!(backend.chain_spec().core_space_chain_id(), 1029);
        assert_eq!(backend.chain_spec().espace_chain_id(), 1030);
        assert_eq!(backend.chain_spec().network_id(), 1029);
    }

    #[tokio::test]
    async fn rejects_core_space_chain_id_mismatch() {
        let error =
            expect_initialization_failure(initialize(mainnet_status(1, 1030, 1029), 1030)).await;

        assert_identity_mismatch(error, identity(1, 1030, 1029, 1030));
    }

    #[tokio::test]
    async fn rejects_core_reported_espace_chain_id_mismatch() {
        let error =
            expect_initialization_failure(initialize(mainnet_status(1029, 1, 1029), 1030)).await;

        assert_identity_mismatch(error, identity(1029, 1, 1029, 1030));
    }

    #[tokio::test]
    async fn rejects_network_id_mismatch() {
        let error =
            expect_initialization_failure(initialize(mainnet_status(1029, 1030, 1), 1030)).await;

        assert_identity_mismatch(error, identity(1029, 1030, 1, 1030));
    }

    #[tokio::test]
    async fn rejects_espace_endpoint_chain_id_mismatch() {
        let error =
            expect_initialization_failure(initialize(mainnet_status(1029, 1030, 1029), 1)).await;

        assert_identity_mismatch(error, identity(1029, 1030, 1029, 1));
    }

    #[tokio::test]
    async fn reports_the_complete_observed_identity() {
        let error = expect_initialization_failure(initialize(mainnet_status(1, 2, 3), 4)).await;

        assert_identity_mismatch(error, identity(1, 2, 3, 4));
    }

    #[test]
    fn rejects_core_status_identity_values_outside_u64() {
        let too_large = U256::from(u64::MAX) + U256::from(1);
        let mut chain_id_status = mainnet_status(1029, 1030, 1029);
        chain_id_status.chain_id = too_large;
        let mut espace_chain_id_status = mainnet_status(1029, 1030, 1029);
        espace_chain_id_status.ethereum_space_chain_id = too_large;
        let mut network_id_status = mainnet_status(1029, 1030, 1029);
        network_id_status.network_id = too_large;

        for (field, status) in [
            (ConfluxCoreStatusIdentityField::ChainId, chain_id_status),
            (
                ConfluxCoreStatusIdentityField::EthereumSpaceChainId,
                espace_chain_id_status,
            ),
            (ConfluxCoreStatusIdentityField::NetworkId, network_id_status),
        ] {
            let error = validate_mainnet_identity(&ConfluxChainSpec::mainnet(), &status, 1030)
                .expect_err("out-of-range identity should reject backend creation");

            assert!(matches!(
                error,
                ConfluxInitializationError::CoreStatusIdentityValueOutOfRange {
                    field: actual_field,
                    actual,
                } if actual_field == field && actual == too_large
            ));
        }
    }

    #[tokio::test]
    async fn preserves_core_status_request_error() {
        let core_asserter = Asserter::new();
        core_asserter.push_failure_msg("Core Space unavailable");
        let espace_asserter = Asserter::new();
        espace_asserter.push_success(&"0x406");
        let (espace_provider, core_space_provider) = mock_providers(espace_asserter, core_asserter);

        let error = expect_initialization_failure(ConfluxSimulationBackend::mainnet(
            espace_provider,
            core_space_provider,
        ))
        .await;

        match error {
            ConfluxInitializationError::CoreStatusRequest {
                source:
                    ConfluxProviderError::JsonRpc {
                        method, message, ..
                    },
            } => {
                assert_eq!(method, "cfx_getStatus");
                assert_eq!(message, "Core Space unavailable");
            }
            unexpected => panic!("unexpected initialization error: {unexpected:?}"),
        }
    }

    #[tokio::test]
    async fn preserves_espace_chain_id_request_error() {
        let core_asserter = Asserter::new();
        core_asserter.push_success(&mainnet_status(1029, 1030, 1029));
        let espace_asserter = Asserter::new();
        espace_asserter.push_failure_msg("eSpace unavailable");
        let (espace_provider, core_space_provider) = mock_providers(espace_asserter, core_asserter);

        let error = expect_initialization_failure(ConfluxSimulationBackend::mainnet(
            espace_provider,
            core_space_provider,
        ))
        .await;

        match error {
            ConfluxInitializationError::EspaceChainIdRequest { source } => {
                assert!(source.to_string().contains("eSpace unavailable"));
            }
            unexpected => panic!("unexpected initialization error: {unexpected:?}"),
        }
    }

    async fn initialize(
        status: CoreStatus,
        espace_chain_id: u64,
    ) -> Result<ConfluxSimulationBackend, ConfluxInitializationError> {
        let core_asserter = Asserter::new();
        core_asserter.push_success(&status);
        let espace_asserter = Asserter::new();
        espace_asserter.push_success(&format!("0x{espace_chain_id:x}"));
        let (espace_provider, core_space_provider) = mock_providers(espace_asserter, core_asserter);

        ConfluxSimulationBackend::mainnet(espace_provider, core_space_provider).await
    }

    async fn expect_initialization_failure(
        result: impl Future<Output = Result<ConfluxSimulationBackend, ConfluxInitializationError>>,
    ) -> ConfluxInitializationError {
        match result.await {
            Ok(_) => panic!("initialization failure should reject backend creation"),
            Err(error) => error,
        }
    }

    fn assert_identity_mismatch(
        error: ConfluxInitializationError,
        actual_identity: ConfluxEndpointIdentity,
    ) {
        assert!(matches!(
            error,
            ConfluxInitializationError::EndpointIdentityMismatch {
                expected,
                actual,
            } if expected == identity(1029, 1030, 1029, 1030) && actual == actual_identity
        ));
    }

    const fn identity(
        core_space_chain_id: u64,
        core_reported_espace_chain_id: u64,
        network_id: u64,
        espace_endpoint_chain_id: u64,
    ) -> ConfluxEndpointIdentity {
        ConfluxEndpointIdentity::new(
            core_space_chain_id,
            core_reported_espace_chain_id,
            network_id,
            espace_endpoint_chain_id,
        )
    }

    fn mock_providers(
        espace_asserter: Asserter,
        core_asserter: Asserter,
    ) -> (DynProvider<Ethereum>, ConfluxProvider) {
        let espace_provider = RootProvider::new(RpcClient::mocked(espace_asserter)).erased();
        let core_space_provider = ConfluxProvider::new(RpcClient::mocked(core_asserter));
        (espace_provider, core_space_provider)
    }

    fn mainnet_status(
        core_space_chain_id: u64,
        espace_chain_id: u64,
        network_id: u64,
    ) -> CoreStatus {
        CoreStatus {
            best_hash: B256::ZERO,
            chain_id: U256::from(core_space_chain_id),
            ethereum_space_chain_id: U256::from(espace_chain_id),
            network_id: U256::from(network_id),
            epoch_number: U256::ZERO,
            block_number: U256::ZERO,
            pending_tx_number: U256::ZERO,
            latest_checkpoint: U256::ZERO,
            latest_confirmed: U256::ZERO,
            latest_state: U256::ZERO,
            latest_finalized: U256::ZERO,
        }
    }
}
