use alloy_primitives::U8;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::CoreSpaceTransactionType;

impl Serialize for CoreSpaceTransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        U8::from(*self as u8).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CoreSpaceTransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = U8::deserialize(deserializer)?;
        Self::try_from(value.to::<u8>()).map_err(de::Error::custom)
    }
}
