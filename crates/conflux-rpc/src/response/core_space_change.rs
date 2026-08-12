use alloy_primitives::Address;
use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{H256, U64, U256};
use conflux_simulation::core_space as simulation_core_space;
use serde::Serialize;

use super::{
    b256_to_wire,
    change::{Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem},
    core_space::{ResponseMappingError, map_core_address},
    u256_to_wire,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "changeType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum Change {
    NativeTransfer {
        from: RpcAddress,
        to: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    NativeBurn {
        from: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    Erc20Transfer {
        contract_address: RpcAddress,
        from: RpcAddress,
        to: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Approval {
        contract_address: RpcAddress,
        owner: RpcAddress,
        spender: RpcAddress,
        approved_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: RpcAddress,
        from: RpcAddress,
        to: RpcAddress,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: RpcAddress,
        owner: RpcAddress,
        approved_address: Option<RpcAddress>,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: RpcAddress,
        owner: RpcAddress,
        operator: RpcAddress,
        approved: bool,
    },
    Erc1155TransferSingle {
        contract_address: RpcAddress,
        operator: RpcAddress,
        from: RpcAddress,
        to: RpcAddress,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: RpcAddress,
        operator: RpcAddress,
        from: RpcAddress,
        to: RpcAddress,
        items: Vec<Erc1155TransferItem>,
    },
    StakingDeposit {
        account: RpcAddress,
        raw_amount: U256,
    },
    StakingWithdrawal {
        account: RpcAddress,
        raw_amount: U256,
        reward_raw_amount: U256,
    },
    StakingBurn {
        account: RpcAddress,
        raw_amount: U256,
    },
    StakingVoteLock {
        account: RpcAddress,
        unlock_block_number: U64,
        required_locked_raw_amount_before: U256,
        required_locked_raw_amount_after: U256,
    },
    PosRegistration {
        account: RpcAddress,
        pos_identifier: H256,
        newly_locked_vote_count: U64,
        newly_locked_raw_amount: U256,
    },
    PosStakeIncrease {
        account: RpcAddress,
        pos_identifier: H256,
        newly_locked_vote_count: U64,
        newly_locked_raw_amount: U256,
    },
    PosRetirementRequest {
        account: RpcAddress,
        pos_identifier: H256,
        requested_vote_count: U64,
    },
    SponsorshipDeposit {
        sponsored_resource: SponsoredResource,
        sponsor: RpcAddress,
        contract_address: RpcAddress,
        raw_amount: U256,
    },
    SponsorshipRefund {
        sponsored_resource: SponsoredResource,
        sponsor: RpcAddress,
        contract_address: RpcAddress,
        raw_amount: U256,
    },
    SponsorshipConfiguration {
        contract_address: RpcAddress,
        #[serde(flatten)]
        configuration: SponsorshipConfiguration,
    },
    SponsorshipEligibilityRule {
        contract_address: RpcAddress,
        applies_to: SponsorshipEligibilityTarget,
        enabled_before: bool,
        enabled_after: bool,
    },
    StoragePointConversion {
        contract_address: RpcAddress,
        converted_cfx_raw_amount: U256,
    },
    CrossSpaceTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeCurrency {
    name: String,
    symbol: String,
    decimals: u8,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum SponsoredResource {
    Gas,
    StorageCollateral,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "sponsoredResource",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum SponsorshipConfiguration {
    Gas {
        sponsor_before: Option<RpcAddress>,
        sponsor_after: Option<RpcAddress>,
        max_sponsored_gas_fee_raw_amount_before: U256,
        max_sponsored_gas_fee_raw_amount_after: U256,
    },
    StorageCollateral {
        sponsor_before: Option<RpcAddress>,
        sponsor_after: Option<RpcAddress>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum SponsorshipEligibilityTarget {
    Account { address: RpcAddress },
    AllAccounts,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "space",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum CrossSpaceAddress {
    CoreSpace { address: RpcAddress },
    Espace { address: Address },
}

pub(super) fn try_map_changes(
    changes: Vec<simulation_core_space::CoreSpaceChange>,
    network: Network,
) -> Result<Vec<Change>, ResponseMappingError> {
    changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| try_map_change(change, network, &format!("changes[{index}]")))
        .collect()
}

fn try_map_change(
    change: simulation_core_space::CoreSpaceChange,
    network: Network,
    field: &str,
) -> Result<Change, ResponseMappingError> {
    use simulation_core_space::CoreSpaceChange as Source;

    Ok(match change {
        Source::NativeTransfer {
            from,
            to,
            raw_amount,
            currency,
        } => Change::NativeTransfer {
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
            raw_amount: u256_to_wire(raw_amount),
            currency: currency.into(),
        },
        Source::NativeBurn {
            from,
            raw_amount,
            currency,
        } => Change::NativeBurn {
            from: map_address(from, network, field, "from")?,
            raw_amount: u256_to_wire(raw_amount),
            currency: currency.into(),
        },
        Source::Standard(change) => try_map_standard_change(change, network, field)?,
        Source::StakingDeposit {
            account,
            raw_amount,
        } => Change::StakingDeposit {
            account: map_address(account, network, field, "account")?,
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::StakingWithdrawal {
            account,
            raw_amount,
            reward_raw_amount,
        } => Change::StakingWithdrawal {
            account: map_address(account, network, field, "account")?,
            raw_amount: u256_to_wire(raw_amount),
            reward_raw_amount: u256_to_wire(reward_raw_amount),
        },
        Source::StakingBurn {
            account,
            raw_amount,
        } => Change::StakingBurn {
            account: map_address(account, network, field, "account")?,
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::StakingVoteLock {
            account,
            unlock_block_number,
            required_locked_raw_amount_before,
            required_locked_raw_amount_after,
        } => Change::StakingVoteLock {
            account: map_address(account, network, field, "account")?,
            unlock_block_number: unlock_block_number.into(),
            required_locked_raw_amount_before: u256_to_wire(required_locked_raw_amount_before),
            required_locked_raw_amount_after: u256_to_wire(required_locked_raw_amount_after),
        },
        Source::PoSRegistration {
            account,
            pos_identifier,
            newly_locked_vote_count,
            newly_locked_raw_amount,
        } => Change::PosRegistration {
            account: map_address(account, network, field, "account")?,
            pos_identifier: b256_to_wire(pos_identifier),
            newly_locked_vote_count: newly_locked_vote_count.into(),
            newly_locked_raw_amount: u256_to_wire(newly_locked_raw_amount),
        },
        Source::PoSStakeIncrease {
            account,
            pos_identifier,
            newly_locked_vote_count,
            newly_locked_raw_amount,
        } => Change::PosStakeIncrease {
            account: map_address(account, network, field, "account")?,
            pos_identifier: b256_to_wire(pos_identifier),
            newly_locked_vote_count: newly_locked_vote_count.into(),
            newly_locked_raw_amount: u256_to_wire(newly_locked_raw_amount),
        },
        Source::PoSRetirementRequest {
            account,
            pos_identifier,
            requested_vote_count,
        } => Change::PosRetirementRequest {
            account: map_address(account, network, field, "account")?,
            pos_identifier: b256_to_wire(pos_identifier),
            requested_vote_count: requested_vote_count.into(),
        },
        Source::SponsorshipDeposit {
            sponsored_resource,
            sponsor,
            contract_address,
            raw_amount,
        } => Change::SponsorshipDeposit {
            sponsored_resource: sponsored_resource.into(),
            sponsor: map_address(sponsor, network, field, "sponsor")?,
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::SponsorshipRefund {
            sponsored_resource,
            sponsor,
            contract_address,
            raw_amount,
        } => Change::SponsorshipRefund {
            sponsored_resource: sponsored_resource.into(),
            sponsor: map_address(sponsor, network, field, "sponsor")?,
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::SponsorshipConfiguration {
            contract_address,
            configuration,
        } => Change::SponsorshipConfiguration {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            configuration: try_map_sponsorship_configuration(configuration, network, field)?,
        },
        Source::SponsorshipEligibilityRule {
            contract_address,
            applies_to,
            enabled_before,
            enabled_after,
        } => Change::SponsorshipEligibilityRule {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            applies_to: try_map_eligibility_target(applies_to, network, field)?,
            enabled_before,
            enabled_after,
        },
        Source::StoragePointConversion {
            contract_address,
            converted_cfx_raw_amount,
        } => Change::StoragePointConversion {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            converted_cfx_raw_amount: u256_to_wire(converted_cfx_raw_amount),
        },
        Source::CrossSpaceTransfer {
            from,
            to,
            raw_amount,
        } => Change::CrossSpaceTransfer {
            from: try_map_cross_space_address(from, network, field, "from")?,
            to: try_map_cross_space_address(to, network, field, "to")?,
            raw_amount: u256_to_wire(raw_amount),
        },
    })
}

fn try_map_standard_change(
    change: simulation_core_space::StandardChange<simulation_core_space::CoreAddress>,
    network: Network,
    field: &str,
) -> Result<Change, ResponseMappingError> {
    use simulation_core_space::StandardChange as Source;

    Ok(match change {
        Source::Erc20Transfer {
            contract_address,
            from,
            to,
            raw_amount,
            metadata,
        } => Change::Erc20Transfer {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
            raw_amount: u256_to_wire(raw_amount),
            metadata: metadata.into(),
        },
        Source::Erc20Approval {
            contract_address,
            owner,
            spender,
            approved_amount,
            metadata,
        } => Change::Erc20Approval {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            owner: map_address(owner, network, field, "owner")?,
            spender: map_address(spender, network, field, "spender")?,
            approved_amount: u256_to_wire(approved_amount),
            metadata: metadata.into(),
        },
        Source::Erc721Transfer {
            contract_address,
            from,
            to,
            token_id,
            metadata,
        } => Change::Erc721Transfer {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
            token_id: u256_to_wire(token_id),
            metadata: metadata.into(),
        },
        Source::Erc721Approval {
            contract_address,
            owner,
            approved_address,
            token_id,
            metadata,
        } => Change::Erc721Approval {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            owner: map_address(owner, network, field, "owner")?,
            approved_address: try_map_optional_address(
                approved_address,
                network,
                field,
                "approvedAddress",
            )?,
            token_id: u256_to_wire(token_id),
            metadata: metadata.into(),
        },
        Source::OperatorApproval {
            contract_address,
            owner,
            operator,
            approved,
        } => Change::OperatorApproval {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            owner: map_address(owner, network, field, "owner")?,
            operator: map_address(operator, network, field, "operator")?,
            approved,
        },
        Source::Erc1155TransferSingle {
            contract_address,
            operator,
            from,
            to,
            token_id,
            raw_amount,
        } => Change::Erc1155TransferSingle {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            operator: map_address(operator, network, field, "operator")?,
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
            token_id: u256_to_wire(token_id),
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::Erc1155TransferBatch {
            contract_address,
            operator,
            from,
            to,
            items,
        } => Change::Erc1155TransferBatch {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            operator: map_address(operator, network, field, "operator")?,
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
            items: items.into_iter().map(Into::into).collect(),
        },
    })
}

fn try_map_sponsorship_configuration(
    configuration: simulation_core_space::SponsorshipConfiguration,
    network: Network,
    field: &str,
) -> Result<SponsorshipConfiguration, ResponseMappingError> {
    Ok(match configuration {
        simulation_core_space::SponsorshipConfiguration::Gas {
            sponsor_before,
            sponsor_after,
            max_sponsored_gas_fee_raw_amount_before,
            max_sponsored_gas_fee_raw_amount_after,
        } => SponsorshipConfiguration::Gas {
            sponsor_before: try_map_optional_address(
                sponsor_before,
                network,
                field,
                "sponsorBefore",
            )?,
            sponsor_after: try_map_optional_address(sponsor_after, network, field, "sponsorAfter")?,
            max_sponsored_gas_fee_raw_amount_before: u256_to_wire(
                max_sponsored_gas_fee_raw_amount_before,
            ),
            max_sponsored_gas_fee_raw_amount_after: u256_to_wire(
                max_sponsored_gas_fee_raw_amount_after,
            ),
        },
        simulation_core_space::SponsorshipConfiguration::StorageCollateral {
            sponsor_before,
            sponsor_after,
        } => SponsorshipConfiguration::StorageCollateral {
            sponsor_before: try_map_optional_address(
                sponsor_before,
                network,
                field,
                "sponsorBefore",
            )?,
            sponsor_after: try_map_optional_address(sponsor_after, network, field, "sponsorAfter")?,
        },
    })
}

fn try_map_eligibility_target(
    target: simulation_core_space::SponsorshipEligibilityTarget,
    network: Network,
    field: &str,
) -> Result<SponsorshipEligibilityTarget, ResponseMappingError> {
    Ok(match target {
        simulation_core_space::SponsorshipEligibilityTarget::Account(address) => {
            SponsorshipEligibilityTarget::Account {
                address: map_address(address, network, field, "appliesTo.address")?,
            }
        }
        simulation_core_space::SponsorshipEligibilityTarget::AllAccounts => {
            SponsorshipEligibilityTarget::AllAccounts
        }
    })
}

fn try_map_cross_space_address(
    address: simulation_core_space::CrossSpaceAddress,
    network: Network,
    field: &str,
    endpoint: &str,
) -> Result<CrossSpaceAddress, ResponseMappingError> {
    Ok(match address {
        simulation_core_space::CrossSpaceAddress::CoreSpace(address) => {
            CrossSpaceAddress::CoreSpace {
                address: map_address(address, network, field, &format!("{endpoint}.address"))?,
            }
        }
        simulation_core_space::CrossSpaceAddress::Espace(address) => {
            CrossSpaceAddress::Espace { address }
        }
    })
}

fn try_map_optional_address(
    address: Option<simulation_core_space::CoreAddress>,
    network: Network,
    field: &str,
    name: &str,
) -> Result<Option<RpcAddress>, ResponseMappingError> {
    address
        .map(|address| map_address(address, network, field, name))
        .transpose()
}

fn map_address(
    address: simulation_core_space::CoreAddress,
    network: Network,
    field: &str,
    name: &str,
) -> Result<RpcAddress, ResponseMappingError> {
    map_core_address(address, network, format!("{field}.{name}"))
}

impl From<simulation_core_space::CoreSpaceNativeCurrency> for NativeCurrency {
    fn from(currency: simulation_core_space::CoreSpaceNativeCurrency) -> Self {
        Self {
            name: currency.name,
            symbol: currency.symbol,
            decimals: currency.decimals,
        }
    }
}

impl From<simulation_core_space::SponsoredResource> for SponsoredResource {
    fn from(resource: simulation_core_space::SponsoredResource) -> Self {
        match resource {
            simulation_core_space::SponsoredResource::Gas => Self::Gas,
            simulation_core_space::SponsoredResource::StorageCollateral => Self::StorageCollateral,
        }
    }
}
