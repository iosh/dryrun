use alloy_eips::BlockNumHash;
use alloy_primitives::B256;
use serde::{Serialize, Serializer};

/// Serializes a block number and hash with a JSON-RPC hex quantity.
pub fn serialize_block<S: Serializer>(
    block: &BlockNumHash,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Block {
        #[serde(with = "alloy_serde::quantity")]
        number: u64,
        hash: B256,
    }

    Block {
        number: block.number,
        hash: block.hash,
    }
    .serialize(serializer)
}
