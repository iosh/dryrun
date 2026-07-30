use std::collections::HashMap;

use alloy_primitives::{Address, Bytes};
use contract_standards::{
    ERC721_METADATA_INTERFACE_ID, Erc20Metadata, Erc721CollectionMetadata, MetadataRequests,
    StandardMetadata, decimals_call, decode_decimals, decode_name, decode_supports_interface,
    decode_symbol, name_call, supports_interface_call, symbol_call,
};
use revm::context_interface::result::EVMError;

use crate::{EvmEngineError, EvmTransaction, NativeMetadata, changes::ChangeMetadata};

use super::{
    MainnetAlloyEvm,
    read_call::{ReadCallOutcome, execute_read_call, with_read_call_context},
};

pub(super) fn load_change_metadata<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    transaction: &EvmTransaction,
    chain_id: u64,
    requests: MetadataRequests,
) -> Result<ChangeMetadata, EvmEngineError> {
    let native = native_metadata(chain_id);

    if requests.is_empty() {
        return Ok(ChangeMetadata::new(native, StandardMetadata::default()));
    }

    with_read_call_context(evm, |evm| {
        MetadataReader {
            evm,
            transaction,
            chain_id,
        }
        .read(native, &requests)
    })
}

struct MetadataReader<'evm, 'transaction, INSP> {
    evm: &'evm mut MainnetAlloyEvm<INSP>,
    transaction: &'transaction EvmTransaction,
    chain_id: u64,
}

fn native_metadata(chain_id: u64) -> NativeMetadata {
    match chain_id {
        1 => NativeMetadata {
            name: Some("Ether".to_string()),
            symbol: Some("ETH".to_string()),
            decimals: Some(18),
        },
        _ => NativeMetadata::default(),
    }
}

impl<INSP> MetadataReader<'_, '_, INSP> {
    fn read(
        &mut self,
        native: NativeMetadata,
        requests: &MetadataRequests,
    ) -> Result<ChangeMetadata, EvmEngineError> {
        let mut erc20 = HashMap::new();
        let mut erc721 = HashMap::new();

        for &contract in requests.erc20_contracts() {
            erc20.insert(contract, self.read_erc20(contract)?);
        }

        for &collection in requests.erc721_collections() {
            erc721.insert(collection, self.read_erc721(collection)?);
        }

        Ok(ChangeMetadata::new(
            native,
            StandardMetadata::new(erc20, erc721),
        ))
    }

    fn read_erc20(&mut self, contract: Address) -> Result<Erc20Metadata, EvmEngineError> {
        Ok(Erc20Metadata {
            name: self.read_optional(contract, name_call(), decode_name)?,
            symbol: self.read_optional(contract, symbol_call(), decode_symbol)?,
            decimals: self.read_optional(contract, decimals_call(), decode_decimals)?,
        })
    }

    fn read_erc721(
        &mut self,
        collection: Address,
    ) -> Result<Erc721CollectionMetadata, EvmEngineError> {
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
    ) -> Result<Option<T>, EvmEngineError> {
        let outcome = execute_read_call(self.evm, self.transaction, self.chain_id, target, data)
            .map_err(|error| match error {
                EVMError::Database(error) => EvmEngineError::state_access_error(format!(
                    "state access failed during metadata read from {target}: {error}"
                )),
                error => EvmEngineError::analysis_failed(format!(
                    "metadata read from {target} failed: {error}"
                )),
            })?;

        Ok(match outcome {
            ReadCallOutcome::Success(output) => decode(output.as_ref()),
            ReadCallOutcome::Revert(_) | ReadCallOutcome::Halt(_) => None,
        })
    }
}
