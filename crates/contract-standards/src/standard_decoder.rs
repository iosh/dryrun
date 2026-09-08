use std::hash::Hash;

use alloy_primitives::{Address, B256, U256};

use crate::{
    Erc1155TransferItem, StandardChange,
    event_codec::{DecodedEvent, StandardEventDecodeError, decode_log},
    metadata::{MetadataValues, MissingMetadataOutcome},
};

/// A successfully decoded standard-log candidate that has not yet been enriched.
///
/// The decoded event is readable for chain-specific verification. Conversion
/// into [`StandardChange`] still requires all metadata call outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedStandardLog<A> {
    pub(crate) event: DecodedStandardEvent<A>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedStandardEvent<A> {
    Erc20Transfer {
        token: A,
        from: A,
        to: A,
        amount: U256,
    },
    Erc20Approval {
        token: A,
        owner: A,
        spender: A,
        value: U256,
    },
    Erc721Transfer {
        collection: A,
        from: A,
        to: A,
        token_id: U256,
    },
    Erc721Approval {
        collection: A,
        owner: A,
        approved_address: Option<A>,
        token_id: U256,
    },
    OperatorApproval {
        collection: A,
        owner: A,
        operator: A,
        approved: bool,
    },
    Erc1155TransferSingle {
        collection: A,
        operator: A,
        from: A,
        to: A,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        collection: A,
        operator: A,
        from: A,
        to: A,
        items: Vec<Erc1155TransferItem>,
    },
}

/// Decodes one committed standard log into a chain-neutral candidate.
///
/// Unknown topics return `Ok(None)`. A supported topic with malformed fields
/// returns an error. The caller owns execution position and can map the raw
/// EVM address into its chain-specific type.
pub fn decode_standard_log<A>(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
    map_address: impl Fn(Address) -> A,
) -> Result<Option<DecodedStandardLog<A>>, StandardEventDecodeError> {
    let Some(event) = decode_log(contract_address, topics, data)? else {
        return Ok(None);
    };

    let event = match event {
        DecodedEvent::Erc20Transfer {
            token,
            from,
            to,
            amount,
        } => DecodedStandardEvent::Erc20Transfer {
            token: map_address(token),
            from: map_address(from),
            to: map_address(to),
            amount,
        },
        DecodedEvent::Erc20Approval {
            token,
            owner,
            spender,
            value,
        } => DecodedStandardEvent::Erc20Approval {
            token: map_address(token),
            owner: map_address(owner),
            spender: map_address(spender),
            value,
        },
        DecodedEvent::Erc721Transfer {
            collection,
            from,
            to,
            token_id,
        } => DecodedStandardEvent::Erc721Transfer {
            collection: map_address(collection),
            from: map_address(from),
            to: map_address(to),
            token_id,
        },
        DecodedEvent::Erc721Approval {
            collection,
            owner,
            approved_address,
            token_id,
        } => DecodedStandardEvent::Erc721Approval {
            collection: map_address(collection),
            owner: map_address(owner),
            approved_address: (approved_address != Address::ZERO)
                .then(|| map_address(approved_address)),
            token_id,
        },
        DecodedEvent::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        } => DecodedStandardEvent::OperatorApproval {
            collection: map_address(collection),
            owner: map_address(owner),
            operator: map_address(operator),
            approved,
        },
        DecodedEvent::Erc1155TransferSingle {
            collection,
            operator,
            from,
            to,
            token_id,
            amount,
        } => DecodedStandardEvent::Erc1155TransferSingle {
            collection: map_address(collection),
            operator: map_address(operator),
            from: map_address(from),
            to: map_address(to),
            token_id,
            amount,
        },
        DecodedEvent::Erc1155TransferBatch {
            collection,
            operator,
            from,
            to,
            items,
        } => DecodedStandardEvent::Erc1155TransferBatch {
            collection: map_address(collection),
            operator: map_address(operator),
            from: map_address(from),
            to: map_address(to),
            items,
        },
    };

    Ok(Some(DecodedStandardLog { event }))
}

impl<A> DecodedStandardLog<A> {
    pub const fn event(&self) -> &DecodedStandardEvent<A> {
        &self.event
    }
}

impl<A> DecodedStandardLog<A>
where
    A: Eq + Hash,
{
    /// Converts this decoded log into a public change after every required
    /// metadata call has a recorded outcome.
    pub fn into_change(
        self,
        metadata: &MetadataValues<A>,
    ) -> Result<StandardChange<A>, MissingMetadataOutcome> {
        Ok(match self.event {
            DecodedStandardEvent::Erc20Transfer {
                token,
                from,
                to,
                amount,
            } => {
                let change_metadata = metadata.erc20_metadata(&token)?;
                StandardChange::Erc20Transfer {
                    contract_address: token,
                    from,
                    to,
                    raw_amount: amount,
                    metadata: change_metadata,
                }
            }
            DecodedStandardEvent::Erc20Approval {
                token,
                owner,
                spender,
                value,
            } => {
                let change_metadata = metadata.erc20_metadata(&token)?;
                StandardChange::Erc20Approval {
                    contract_address: token,
                    owner,
                    spender,
                    approved_amount: value,
                    metadata: change_metadata,
                }
            }
            DecodedStandardEvent::Erc721Transfer {
                collection,
                from,
                to,
                token_id,
            } => {
                let change_metadata = metadata.erc721(&collection)?;
                StandardChange::Erc721Transfer {
                    contract_address: collection,
                    from,
                    to,
                    token_id,
                    metadata: change_metadata,
                }
            }
            DecodedStandardEvent::Erc721Approval {
                collection,
                owner,
                approved_address,
                token_id,
            } => {
                let change_metadata = metadata.erc721(&collection)?;
                StandardChange::Erc721Approval {
                    contract_address: collection,
                    owner,
                    approved_address,
                    token_id,
                    metadata: change_metadata,
                }
            }
            DecodedStandardEvent::OperatorApproval {
                collection,
                owner,
                operator,
                approved,
            } => StandardChange::OperatorApproval {
                contract_address: collection,
                owner,
                operator,
                approved,
            },
            DecodedStandardEvent::Erc1155TransferSingle {
                collection,
                operator,
                from,
                to,
                token_id,
                amount,
            } => StandardChange::Erc1155TransferSingle {
                contract_address: collection,
                operator,
                from,
                to,
                token_id,
                raw_amount: amount,
            },
            DecodedStandardEvent::Erc1155TransferBatch {
                collection,
                operator,
                from,
                to,
                items,
            } => StandardChange::Erc1155TransferBatch {
                contract_address: collection,
                operator,
                from,
                to,
                items,
            },
        })
    }
}
