use crate::state::provider::RemoteStateProviderError;

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
