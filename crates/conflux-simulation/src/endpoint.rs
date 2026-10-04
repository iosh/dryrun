use alloy::{
    primitives::U256,
    providers::{DynProvider, Provider},
};
use cfx_types::Space;
use conflux_provider::ConfluxProvider;

use crate::{ChainSpec, Error};

/// Checks the network identities before a simulator uses either endpoint.
pub(crate) async fn check_network(
    chain: &ChainSpec,
    core: &ConfluxProvider,
    espace: &DynProvider,
) -> Result<(), Error> {
    // Supported chain specifications use constant chain IDs from genesis.
    let expected_espace_chain_id = u64::from(chain.params.chain_id(0, Space::Ethereum));
    let expected_core_chain_id = u64::from(chain.params.chain_id(0, Space::Native));
    let espace_chain_id = espace
        .get_chain_id()
        .await
        .map_err(|source| Error::EspaceProvider {
            operation: "eth_chainId",
            source,
        })?;
    check_id(
        "eSpace",
        "chainId",
        U256::from(espace_chain_id),
        expected_espace_chain_id,
    )?;

    let core_status = core.cfx_get_status().await?;
    check_id(
        "Core Space",
        "chainId",
        core_status.chain_id,
        expected_core_chain_id,
    )?;
    check_id(
        "Core Space",
        "ethereumSpaceChainId",
        core_status.ethereum_space_chain_id,
        expected_espace_chain_id,
    )?;
    check_id(
        "Core Space",
        "networkId",
        core_status.network_id,
        chain.params.network_id,
    )
}

fn check_id(
    endpoint: &'static str,
    field: &'static str,
    actual: U256,
    expected: u64,
) -> Result<(), Error> {
    if actual != U256::from(expected) {
        return Err(Error::ChainMismatch {
            endpoint,
            field,
            expected,
            actual,
        });
    }
    Ok(())
}
