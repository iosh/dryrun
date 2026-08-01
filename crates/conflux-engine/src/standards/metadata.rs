use std::collections::HashMap;

use alloy_primitives::{Address, Bytes};
use cfx_executor::{machine::Machine, state::State};
use contract_standards::{
    ERC721_METADATA_INTERFACE_ID, Erc20Metadata, Erc721CollectionMetadata, MetadataRequests,
    StandardMetadata, decimals_call, decode_decimals, decode_name, decode_supports_interface,
    decode_symbol, name_call, supports_interface_call, symbol_call,
};
use simulation_changes::{ChangeMetadata, NativeMetadata};

use crate::{ConfluxEngineError, execution::PreparedTransactionExecution};

use super::{StandardReadCallOutcome, execute_standard_read_call};

pub(crate) fn load_change_metadata(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    requests: &MetadataRequests,
) -> Result<ChangeMetadata, ConfluxEngineError> {
    let mut reader = MetadataReader {
        state,
        machine,
        prepared_execution,
    };
    let mut erc20 = HashMap::new();
    let mut erc721 = HashMap::new();

    for &contract in requests.erc20_contracts() {
        erc20.insert(contract, reader.read_erc20(contract)?);
    }

    for &collection in requests.erc721_collections() {
        erc721.insert(collection, reader.read_erc721(collection)?);
    }

    Ok(ChangeMetadata::new(
        NativeMetadata {
            name: Some("Conflux".to_owned()),
            symbol: Some("CFX".to_owned()),
            decimals: Some(18),
        },
        StandardMetadata::new(erc20, erc721),
    ))
}

struct MetadataReader<'a> {
    state: &'a mut State,
    machine: &'a Machine,
    prepared_execution: &'a PreparedTransactionExecution,
}

impl MetadataReader<'_> {
    fn read_erc20(&mut self, contract: Address) -> Result<Erc20Metadata, ConfluxEngineError> {
        Ok(Erc20Metadata {
            name: self.read_optional(contract, name_call(), decode_name)?,
            symbol: self.read_optional(contract, symbol_call(), decode_symbol)?,
            decimals: self.read_optional(contract, decimals_call(), decode_decimals)?,
        })
    }

    fn read_erc721(
        &mut self,
        collection: Address,
    ) -> Result<Erc721CollectionMetadata, ConfluxEngineError> {
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
        target_contract: Address,
        call_data: Bytes,
        decode_return_data: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Result<Option<T>, ConfluxEngineError> {
        let getter_outcome = execute_standard_read_call(
            self.state,
            self.machine,
            self.prepared_execution,
            target_contract,
            call_data,
        )?;

        Ok(match getter_outcome {
            StandardReadCallOutcome::Success(return_data) => {
                decode_return_data(return_data.as_ref())
            }
            StandardReadCallOutcome::Revert | StandardReadCallOutcome::Halt(_) => None,
        })
    }
}
