use crate::transaction::SignedAuthorization;
use alloy_primitives::{U8, U64};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap, SerializeSeq},
};

struct AuthorizationRef<'a>(&'a SignedAuthorization);
impl Serialize for AuthorizationRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(6))?;
        let inner = self.0.inner();
        map.serialize_entry("chainId", &inner.chain_id)?;
        map.serialize_entry("address", &inner.address)?;
        map.serialize_entry("nonce", &U64::from(inner.nonce))?;
        map.serialize_entry("yParity", &U8::from(self.0.y_parity()))?;
        map.serialize_entry("r", &self.0.r())?;
        map.serialize_entry("s", &self.0.s())?;
        map.end()
    }
}

pub(crate) fn serialize_authorizations<S: Serializer>(
    items: &[SignedAuthorization],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(items.len()))?;
    for item in items {
        sequence.serialize_element(&AuthorizationRef(item))?;
    }
    sequence.end()
}
