use std::{borrow::Cow, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

const CHARSET: &[u8; 32] = b"abcdefghjkmnprstuvwxyz0123456789";
const VERSION: u8 = 0;
const MAX_NETWORK_ID: u64 = u32::MAX as u64;

const fn charset_index() -> [i8; 128] {
    let mut index = [-1_i8; 128];
    let mut value = 0;
    while value < CHARSET.len() {
        let character = CHARSET[value];
        index[character as usize] = value as i8;
        if character >= b'a' && character <= b'z' {
            index[(character - b'a' + b'A') as usize] = value as i8;
        }
        value += 1;
    }
    index
}

const CHARSET_INDEX: [i8; 128] = charset_index();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Network {
    Main,
    Test,
    Id(u64),
}

impl Network {
    pub fn from_prefix(prefix: &str) -> Result<Self, NetworkError> {
        match prefix {
            "cfx" => Ok(Self::Main),
            "cfxtest" => Ok(Self::Test),
            prefix if prefix.starts_with("net") => {
                let id = prefix[3..]
                    .parse::<u64>()
                    .map_err(|_| NetworkError::InvalidPrefix(prefix.to_owned()))?;
                if id > MAX_NETWORK_ID {
                    return Err(NetworkError::OutOfRange(id));
                }
                if matches!(id, 1 | 1029) {
                    return Err(NetworkError::ReservedId(id));
                }
                Ok(Self::Id(id))
            }
            prefix => Err(NetworkError::InvalidPrefix(prefix.to_owned())),
        }
    }

    pub fn to_prefix(self) -> Result<String, NetworkError> {
        match self {
            Self::Main => Ok("cfx".to_owned()),
            Self::Test => Ok("cfxtest".to_owned()),
            Self::Id(id) if matches!(id, 1 | 1029) => Err(NetworkError::ReservedId(id)),
            Self::Id(id) if id > MAX_NETWORK_ID => Err(NetworkError::OutOfRange(id)),
            Self::Id(id) => Ok(format!("net{id}")),
        }
    }

    pub(crate) fn validate(self) -> Result<(), NetworkError> {
        self.to_prefix().map(|_| ())
    }
}

impl fmt::Display for Network {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_prefix() {
            Ok(prefix) => formatter.write_str(&prefix),
            Err(error) => write!(formatter, "{error}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NetworkError {
    #[error("invalid Core Space network prefix {0:?}")]
    InvalidPrefix(String),
    #[error("reserved Core Space network id {0}")]
    ReservedId(u64),
    #[error("Core Space network id {0} exceeds the CIP-37 32-bit range")]
    OutOfRange(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoreAddress {
    bytes: [u8; 20],
    network: Network,
}

impl CoreAddress {
    pub fn from_bytes(bytes: [u8; 20], network: Network) -> Result<Self, AddressError> {
        network.validate().map_err(AddressError::Network)?;
        Ok(Self { bytes, network })
    }

    pub fn parse(value: &str) -> Result<Self, AddressError> {
        decode(value)
    }

    pub const fn bytes(self) -> [u8; 20] {
        self.bytes
    }

    pub const fn network(self) -> Network {
        self.network
    }

    pub fn to_cip37(self) -> String {
        encode(self.bytes, self.network).expect("CoreAddress always contains a valid network")
    }
}

impl fmt::Display for CoreAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_cip37())
    }
}

impl FromStr for CoreAddress {
    type Err = AddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for CoreAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_cip37())
    }
}

impl<'de> Deserialize<'de> for CoreAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AddressError {
    #[error("mixed-case CIP-37 address")]
    MixedCase,
    #[error("CIP-37 address is missing a network prefix")]
    MissingPrefix,
    #[error("invalid CIP-37 address network: {0}")]
    Network(#[source] NetworkError),
    #[error("invalid CIP-37 address option {0:?}")]
    InvalidOption(String),
    #[error("invalid CIP-37 address character {0:?}")]
    InvalidCharacter(char),
    #[error("invalid CIP-37 address length {0}")]
    InvalidLength(usize),
    #[error("invalid CIP-37 address checksum {0:#x}")]
    InvalidChecksum(u64),
    #[error("invalid CIP-37 address padding")]
    InvalidPadding,
    #[error("unrecognized CIP-37 version byte {0:#x}")]
    InvalidVersion(u8),
    #[error("CIP-37 address type mismatch: expected {expected}, got {actual}")]
    AddressTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressType {
    Builtin,
    Contract,
    Null,
    User,
    Unknown,
}

impl AddressType {
    fn parse(value: &str) -> Self {
        match value {
            "builtin" => Self::Builtin,
            "contract" => Self::Contract,
            "null" => Self::Null,
            "user" => Self::User,
            _ => Self::Unknown,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Contract => "contract",
            Self::Null => "null",
            Self::User => "user",
            Self::Unknown => "unknown",
        }
    }
}

fn encode(bytes: [u8; 20], network: Network) -> Result<String, NetworkError> {
    let prefix = network.to_prefix()?;
    let mut payload = [0_u8; 21];
    payload[1..].copy_from_slice(&bytes);
    let payload = convert_bits(&payload, 8, 5, true).expect("fixed-size CIP-37 payload");
    let mut checksum_input = expand_prefix(&prefix);
    checksum_input.extend_from_slice(&payload);
    checksum_input.extend_from_slice(&[0; 8]);
    let checksum = polymod(&checksum_input);

    let mut encoded = String::with_capacity(prefix.len() + 1 + payload.len() + 8);
    encoded.push_str(&prefix);
    encoded.push(':');
    for value in payload {
        encoded.push(CHARSET[value as usize] as char);
    }
    for index in (0..8).rev() {
        encoded.push(CHARSET[((checksum >> (index * 5)) & 31) as usize] as char);
    }
    Ok(encoded)
}

fn decode(value: &str) -> Result<CoreAddress, AddressError> {
    let has_lowercase = value.chars().any(char::is_lowercase);
    let has_uppercase = value.chars().any(char::is_uppercase);
    if has_lowercase && has_uppercase {
        return Err(AddressError::MixedCase);
    }

    let value = if has_uppercase {
        Cow::Owned(value.to_ascii_lowercase())
    } else {
        Cow::Borrowed(value)
    };
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() < 2 {
        return Err(AddressError::MissingPrefix);
    }

    let network = Network::from_prefix(parts[0]).map_err(AddressError::Network)?;
    let mut address_type = None;
    for option in &parts[1..parts.len() - 1] {
        let mut fields = option.split('.');
        let key = fields.next();
        let option_value = fields.next();
        if key.is_none() || option_value.is_none() || fields.next().is_some() {
            return Err(AddressError::InvalidOption((*option).to_owned()));
        }
        if key == Some("type") {
            if address_type.is_some() {
                return Err(AddressError::InvalidOption((*option).to_owned()));
            }
            address_type = Some(AddressType::parse(option_value.unwrap()));
        }
    }

    let payload = parts[parts.len() - 1];
    if payload.is_empty() {
        return Err(AddressError::InvalidLength(0));
    }
    let values = payload
        .chars()
        .map(|character| {
            let value = usize::try_from(character as u32)
                .ok()
                .and_then(|value| CHARSET_INDEX.get(value).copied())
                .unwrap_or(-1);
            if value < 0 {
                Err(AddressError::InvalidCharacter(character))
            } else {
                Ok(value as u8)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() < 8 {
        return Err(AddressError::InvalidLength(values.len()));
    }

    let mut checksum_input = expand_prefix(parts[0]);
    checksum_input.extend_from_slice(&values);
    let checksum = polymod(&checksum_input);
    if checksum != 0 {
        return Err(AddressError::InvalidChecksum(checksum));
    }

    let payload = convert_bits(&values[..values.len() - 8], 5, 8, false)?;
    if payload.len() != 21 {
        return Err(AddressError::InvalidLength(payload.len()));
    }
    if payload[0] != VERSION {
        return Err(AddressError::InvalidVersion(payload[0]));
    }

    let mut bytes = [0_u8; 20];
    bytes.copy_from_slice(&payload[1..]);
    if let Some(expected) = address_type {
        let actual = address_type_for(bytes);
        if actual != expected {
            return Err(AddressError::AddressTypeMismatch {
                expected: expected.as_str(),
                actual: actual.as_str(),
            });
        }
    }

    Ok(CoreAddress { bytes, network })
}

fn address_type_for(bytes: [u8; 20]) -> AddressType {
    if bytes.iter().all(|byte| *byte == 0) {
        AddressType::Null
    } else {
        match bytes[0] >> 4 {
            0 => AddressType::Builtin,
            1 => AddressType::User,
            8 => AddressType::Contract,
            _ => AddressType::Unknown,
        }
    }
}

fn expand_prefix(prefix: &str) -> Vec<u8> {
    prefix.bytes().map(|byte| byte & 31).chain([0]).collect()
}

fn polymod(values: &[u8]) -> u64 {
    let mut checksum = 1_u64;
    for value in values {
        let top = checksum >> 35;
        checksum = ((checksum & 0x07ff_ffff_ff) << 5) ^ u64::from(*value);
        if top & 0x01 != 0 {
            checksum ^= 0x98f2_bc8e_61;
        }
        if top & 0x02 != 0 {
            checksum ^= 0x79b7_6d99_e2;
        }
        if top & 0x04 != 0 {
            checksum ^= 0xf33e_5fb3_c4;
        }
        if top & 0x08 != 0 {
            checksum ^= 0xae2e_abe2_a8;
        }
        if top & 0x10 != 0 {
            checksum ^= 0x1e4f_43e4_70;
        }
    }
    checksum ^ 1
}

fn convert_bits(
    values: &[u8],
    from_bits: u8,
    to_bits: u8,
    pad: bool,
) -> Result<Vec<u8>, AddressError> {
    let max_value = (1_u16 << to_bits) - 1;
    let max_input = (1_u16 << from_bits) - 1;
    let mut accumulator = 0_u16;
    let mut bits = 0_u8;
    let mut result = Vec::new();

    for value in values {
        if u16::from(*value) > max_input {
            return Err(AddressError::InvalidPadding);
        }
        accumulator = (accumulator << from_bits) | u16::from(*value);
        bits += from_bits;
        while bits >= to_bits {
            bits -= to_bits;
            result.push(((accumulator >> bits) & max_value) as u8);
            accumulator &= !(max_value << bits);
        }
    }

    if pad {
        if bits != 0 {
            result.push(((accumulator << (to_bits - bits)) & max_value) as u8);
        }
    } else if bits >= from_bits || accumulator != 0 {
        return Err(AddressError::InvalidPadding);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{CoreAddress, Network};

    #[test]
    fn cip37_official_vectors_round_trip() {
        let vectors = [
            (
                "85d80245dc02f5a89589e1f19c5c718e405b56cd",
                Network::Main,
                "cfx:acc7uawf5ubtnmezvhu9dhc6sghea0403y2dgpyfjp",
            ),
            (
                "85d80245dc02f5a89589e1f19c5c718e405b56cd",
                Network::Test,
                "cfxtest:acc7uawf5ubtnmezvhu9dhc6sghea0403ywjz6wtpg",
            ),
            (
                "1a2f80341409639ea6a35bbcab8299066109aa55",
                Network::Main,
                "cfx:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4xg",
            ),
            (
                "1a2f80341409639ea6a35bbcab8299066109aa55",
                Network::Test,
                "cfxtest:aarc9abycue0hhzgyrr53m6cxedgccrmmy8m50bu1p",
            ),
        ];

        for (hex, network, encoded) in vectors {
            let mut bytes = [0_u8; 20];
            for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                bytes[index] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
            }
            let address = CoreAddress::from_bytes(bytes, network).unwrap();
            assert_eq!(address.to_cip37(), encoded);
            assert_eq!(CoreAddress::parse(encoded).unwrap(), address);
        }

        assert_eq!(
            CoreAddress::parse("CFX:TYPE.USER:AARC9ABYCUE0HHZGYRR53M6CXEDGCCRMMYYBJGH4XG")
                .unwrap()
                .to_cip37(),
            "cfx:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4xg"
        );
    }

    #[test]
    fn cip37_rejects_invalid_input_and_network() {
        assert!(CoreAddress::parse("cfx:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4xj").is_err());
        assert!(CoreAddress::parse("cfx:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4Xg").is_err());
        assert!(CoreAddress::parse("net1:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4xg").is_err());
        assert!(
            CoreAddress::parse("net4294967296:aarc9abycue0hhzgyrr53m6cxedgccrmmyybjgh4xg").is_err()
        );
        assert!(CoreAddress::from_bytes([0; 20], Network::Id(1029)).is_err());
        assert!(CoreAddress::from_bytes([0; 20], Network::Id(4_294_967_296)).is_err());
    }
}
