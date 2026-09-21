use std::{collections::HashSet, fmt};

use alloy_primitives::{Address, B256, U8, U64, U256};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, IgnoredAny, MapAccess, Visitor},
};

use crate::transaction::{
    AccessListItem, Authorization, FeeInput, PartialTransactionCommon,
    SignedAuthorization as Eip7702SignedAuthorization, TransactionRequest, TxType,
};

pub(crate) fn serialize_transaction_type<S>(
    transaction_type: &Option<TxType>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    transaction_type
        .map(|transaction_type| U8::from(transaction_type as u8))
        .serialize(serializer)
}

impl<'de, A> Deserialize<'de> for AccessListItem<A>
where
    A: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Item<A> {
            address: A,
            storage_keys: Vec<B256>,
        }

        let item = Item::deserialize(deserializer)?;
        Ok(Self {
            address: item.address,
            storage_keys: item.storage_keys,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedAuthorization {
    chain_id: U256,
    address: Address,
    nonce: U64,
    y_parity: Option<U8>,
    v: Option<U8>,
    r: U256,
    s: U256,
}

impl SignedAuthorization {
    fn into_inner<E>(self) -> Result<Eip7702SignedAuthorization, E>
    where
        E: de::Error,
    {
        let y_parity = match (self.y_parity, self.v) {
            (Some(y_parity), Some(v)) if y_parity != v => {
                return Err(E::custom("yParity and v must agree"));
            }
            (Some(value), _) | (_, Some(value)) => value,
            (None, None) => return Err(E::missing_field("yParity")),
        };
        Ok(Eip7702SignedAuthorization::new_unchecked(
            Authorization {
                chain_id: self.chain_id,
                address: self.address,
                nonce: self.nonce.to(),
            },
            y_parity.to(),
            self.r,
            self.s,
        ))
    }
}

#[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq)]
#[serde(field_identifier, rename_all = "camelCase")]
enum Field {
    From,
    To,
    Nonce,
    Gas,
    Value,
    Input,
    Data,
    ChainId,
    GasPrice,
    MaxFeePerGas,
    MaxPriorityFeePerGas,
    MaxFeePerBlobGas,
    #[serde(rename = "type")]
    Type,
    AccessList,
    BlobVersionedHashes,
    AuthorizationList,
    #[serde(other)]
    Unknown,
}

impl Field {
    fn name(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::To => "to",
            Self::Nonce => "nonce",
            Self::Gas => "gas",
            Self::Value => "value",
            Self::Input => "input",
            Self::Data => "data",
            Self::ChainId => "chainId",
            Self::GasPrice => "gasPrice",
            Self::MaxFeePerGas => "maxFeePerGas",
            Self::MaxPriorityFeePerGas => "maxPriorityFeePerGas",
            Self::MaxFeePerBlobGas => "maxFeePerBlobGas",
            Self::Type => "type",
            Self::AccessList => "accessList",
            Self::BlobVersionedHashes => "blobVersionedHashes",
            Self::AuthorizationList => "authorizationList",
            Self::Unknown => unreachable!("unknown fields are ignored"),
        }
    }
}

impl<'de> Deserialize<'de> for TransactionRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TransactionRequestVisitor;

        impl<'de> Visitor<'de> for TransactionRequestVisitor {
            type Value = TransactionRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a transaction object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let (
                    mut from,
                    mut to,
                    mut nonce,
                    mut gas_limit,
                    mut value,
                    mut input,
                    mut legacy_input,
                    mut chain_id,
                ) = (None, None, None, None, None, None, None, None);
                let mut fees = FeeInput::default();
                let (
                    mut transaction_type,
                    mut max_fee_per_blob_gas,
                    mut access_list,
                    mut blob_versioned_hashes,
                    mut authorization_list,
                ) = (None, None, None, None, None);
                let mut seen = HashSet::new();

                while let Some(field) = map.next_key::<Field>()? {
                    if field == Field::Unknown {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    }
                    if !seen.insert(field) {
                        return Err(de::Error::duplicate_field(field.name()));
                    }
                    match field {
                        Field::From => from = map.next_value()?,
                        Field::To => to = map.next_value()?,
                        Field::Nonce => {
                            nonce = map
                                .next_value::<Option<U64>>()?
                                .map(|quantity| quantity.to())
                        }
                        Field::Gas => {
                            gas_limit = map
                                .next_value::<Option<U64>>()?
                                .map(|quantity| quantity.to())
                        }
                        Field::Value => value = map.next_value()?,
                        Field::Input => input = map.next_value()?,
                        Field::Data => legacy_input = map.next_value()?,
                        Field::ChainId => {
                            chain_id = map
                                .next_value::<Option<U64>>()?
                                .map(|quantity| quantity.to())
                        }
                        Field::GasPrice => fees.gas_price = map.next_value()?,
                        Field::MaxFeePerGas => fees.max_fee_per_gas = map.next_value()?,
                        Field::MaxPriorityFeePerGas => {
                            fees.max_priority_fee_per_gas = map.next_value()?
                        }
                        Field::MaxFeePerBlobGas => max_fee_per_blob_gas = map.next_value()?,
                        Field::Type => {
                            transaction_type = map
                                .next_value::<Option<U8>>()?
                                .map(|quantity| TxType::try_from(quantity.to::<u8>()))
                                .transpose()
                                .map_err(de::Error::custom)?
                        }
                        Field::AccessList => access_list = map.next_value()?,
                        Field::BlobVersionedHashes => blob_versioned_hashes = map.next_value()?,
                        Field::AuthorizationList => {
                            authorization_list = map
                                .next_value::<Option<Vec<SignedAuthorization>>>()?
                                .map(|items| {
                                    items
                                        .into_iter()
                                        .map(SignedAuthorization::into_inner)
                                        .collect()
                                })
                                .transpose()?
                        }
                        Field::Unknown => unreachable!("unknown fields are ignored"),
                    }
                }

                if input
                    .as_ref()
                    .zip(legacy_input.as_ref())
                    .is_some_and(|(input, data)| input != data)
                {
                    return Err(de::Error::custom("input and data must agree"));
                }

                Ok(TransactionRequest {
                    common: PartialTransactionCommon {
                        from: from.ok_or_else(|| de::Error::missing_field("from"))?,
                        to,
                        nonce,
                        gas_limit,
                        value,
                        input: input.or(legacy_input),
                        chain_id,
                    },
                    fees,
                    transaction_type,
                    max_fee_per_blob_gas,
                    access_list,
                    blob_versioned_hashes,
                    authorization_list,
                })
            }
        }

        deserializer.deserialize_map(TransactionRequestVisitor)
    }
}
