use cfx_rpc_primitives::Bytes as RpcBytes;
use cfx_types::U256;

use crate::state::provider::RemoteStateProviderError;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RpcStorageWord(Option<RpcBytes>);

impl RpcStorageWord {
    pub(crate) fn into_option_u256(self) -> Result<Option<U256>, RemoteStateProviderError> {
        let Some(value) = self.0 else {
            return Ok(None);
        };

        if value.is_empty() {
            return Ok(None);
        }

        if value.len() != 32 {
            return Err(RemoteStateProviderError::RpcDecode {
                field: "cfx_getStorageAt",
                value: format!("0x{}", hex::encode(value.as_ref())),
                message: format!("expected 32 bytes, got {}", value.len()),
            });
        }

        Ok(Some(U256::from_big_endian(value.as_ref())))
    }
}

pub(crate) fn decode_rpc_bytes(
    value: String,
    field: &'static str,
) -> Result<Vec<u8>, RemoteStateProviderError> {
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| RemoteStateProviderError::RpcDecode {
            field,
            value: value.clone(),
            message: "missing 0x prefix".to_owned(),
        })?;

    if digits.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(digits).map_err(|error| RemoteStateProviderError::RpcDecode {
        field,
        value,
        message: error.to_string(),
    })
}
