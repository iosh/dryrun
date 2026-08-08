use std::collections::HashMap;

use alloy_primitives::{Address, Bytes};
use contract_standards::legacy::{
    ERC721_METADATA_INTERFACE_ID, MetadataRequests, StandardMetadata, decimals_call,
    decode_decimals, decode_name, decode_supports_interface, decode_symbol, name_call,
    supports_interface_call, symbol_call,
};
use contract_standards::{Erc20Metadata, Erc721CollectionMetadata};
use revm::context_interface::result::EVMError;

use crate::EvmSimulationError;
use simulation_transaction::Transaction as EvmTransaction;

use super::read_call::{ReadCallOutcome, execute_read_call, with_read_call_context};
use crate::execution::MainnetEvm;

pub(crate) fn load_standard_metadata<INSP>(
    evm: &mut MainnetEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: MetadataRequests,
) -> Result<StandardMetadata, EvmSimulationError> {
    if requests.is_empty() {
        return Ok(StandardMetadata::default());
    }

    with_read_call_context(evm, |evm| {
        MetadataReader {
            evm,
            transaction,
            chain_id,
        }
        .read(&requests)
    })
}

struct MetadataReader<'evm, 'transaction, INSP> {
    evm: &'evm mut MainnetEvm<INSP>,
    transaction: &'transaction EvmTransaction,
    chain_id: u64,
}

impl<INSP> MetadataReader<'_, '_, INSP> {
    fn read(
        &mut self,
        requests: &MetadataRequests,
    ) -> Result<StandardMetadata, EvmSimulationError> {
        let mut erc20 = HashMap::new();
        let mut erc721 = HashMap::new();

        for &contract in requests.erc20_contracts() {
            erc20.insert(contract, self.read_erc20(contract)?);
        }

        for &collection in requests.erc721_collections() {
            erc721.insert(collection, self.read_erc721(collection)?);
        }

        Ok(StandardMetadata::new(erc20, erc721))
    }

    fn read_erc20(&mut self, contract: Address) -> Result<Erc20Metadata, EvmSimulationError> {
        Ok(Erc20Metadata {
            name: self.read_optional(contract, name_call(), decode_name)?,
            symbol: self.read_optional(contract, symbol_call(), decode_symbol)?,
            decimals: self.read_optional(contract, decimals_call(), decode_decimals)?,
        })
    }

    fn read_erc721(
        &mut self,
        collection: Address,
    ) -> Result<Erc721CollectionMetadata, EvmSimulationError> {
        let supports_metadata = self.read_optional(
            collection,
            supports_interface_call(ERC721_METADATA_INTERFACE_ID),
            decode_supports_interface,
        )?;

        if supports_metadata != Some(true) {
            return Ok(Erc721CollectionMetadata::default());
        }

        Ok(Erc721CollectionMetadata {
            name: self.read_optional(collection, name_call(), decode_name)?,
            symbol: self.read_optional(collection, symbol_call(), decode_symbol)?,
        })
    }

    fn read_optional<T>(
        &mut self,
        target: Address,
        data: Bytes,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Result<Option<T>, EvmSimulationError> {
        let outcome = execute_read_call(self.evm, self.transaction, self.chain_id, target, data)
            .map_err(|error| match error {
                EVMError::Database(error) => EvmSimulationError::state_access_error(format!(
                    "state access failed during metadata read from {target}: {error}"
                )),
                error => EvmSimulationError::analysis_failed(format!(
                    "metadata read from {target} failed: {error}"
                )),
            })?;

        Ok(match outcome {
            ReadCallOutcome::Success(output) => decode(output.as_ref()),
            ReadCallOutcome::Revert(_) | ReadCallOutcome::Halt(_) => None,
        })
    }
}
