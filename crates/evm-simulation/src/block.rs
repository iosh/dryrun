use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    providers::{DynProvider, Provider},
    rpc::types::Header,
};

use crate::Error;

/// Fetches the header of the block the simulation runs on top of.
pub(crate) async fn fetch_header(provider: &DynProvider, block: BlockId) -> Result<Header, Error> {
    if matches!(
        block,
        BlockId::Number(BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest)
    ) {
        return Err(Error::Unsupported(format!(
            "block {block} is not supported"
        )));
    }
    let block_data = provider
        .get_block(block)
        .await
        .map_err(|source| Error::Provider {
            operation: "eth_getBlock",
            source,
        })?
        .ok_or(Error::BlockNotFound(block))?;
    Ok(block_data.header)
}
