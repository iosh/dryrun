use std::convert::TryFrom;

use evm_simulation::{EvmBlockSelector, EvmSimulationRequest};

use crate::{errors::ValidationError, interface as rpc};

use super::{shared::parse_u64_param, transaction::map_transaction};

impl TryFrom<rpc::EvmSimulateTransactionRequest> for EvmSimulationRequest {
    type Error = ValidationError;

    fn try_from(request: rpc::EvmSimulateTransactionRequest) -> Result<Self, Self::Error> {
        request.validate()?;

        let rpc::EvmSimulateTransactionRequest {
            block, transaction, ..
        } = request;

        Ok(Self {
            block: block
                .map(map_block_ref)
                .transpose()?
                .unwrap_or(EvmBlockSelector::Latest),
            transaction: map_transaction(transaction)?,
        })
    }
}

fn map_block_ref(block: rpc::BlockRef) -> Result<EvmBlockSelector, ValidationError> {
    match block {
        rpc::BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(EvmBlockSelector::Latest),
            "safe" => Ok(EvmBlockSelector::Safe),
            "finalized" => Ok(EvmBlockSelector::Finalized),
            value => Ok(EvmBlockSelector::Number(parse_u64_param(value, "block")?)),
        },
        rpc::BlockRef::Hash(_) => Err(ValidationError::not_supported(
            "`block.blockHash` is not supported yet",
        )),
    }
}
