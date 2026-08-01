use alloy_primitives::Address;
use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{H256, U64, U256};
use conflux_service::core_space as service_core_space;
use serde::Serialize;

use super::{
    b256_to_wire,
    change::{Erc20Metadata, Erc721CollectionMetadata, NativeMetadata},
    core_space::{ResponseMappingError, map_core_space_address},
    u256_to_wire,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "changeType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum Change {
    Transfer {
        #[serde(flatten)]
        asset: TransferAsset,
        from: RpcAddress,
        to: RpcAddress,
    },
    Mint {
        #[serde(flatten)]
        asset: MintAsset,
        to: RpcAddress,
    },
    Burn {
        #[serde(flatten)]
        asset: BurnAsset,
        from: RpcAddress,
    },
    Allowance {
        #[serde(flatten)]
        asset: AllowanceAsset,
        owner: RpcAddress,
        spender: RpcAddress,
    },
    TokenApproval {
        #[serde(flatten)]
        asset: TokenApprovalAsset,
    },
    OperatorApproval {
        #[serde(flatten)]
        asset: OperatorApprovalAsset,
        owner: RpcAddress,
        operator: RpcAddress,
        approved_before: bool,
        approved_after: bool,
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
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum TransferAsset {
    Native {
        raw_amount: U256,
        #[serde(flatten)]
        metadata: NativeMetadata,
    },
    Erc20 {
        contract_address: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: RpcAddress,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: RpcAddress,
        token_id: U256,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum MintAsset {
    Erc20 {
        contract_address: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: RpcAddress,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: RpcAddress,
        token_id: U256,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum BurnAsset {
    Native {
        raw_amount: U256,
        #[serde(flatten)]
        metadata: NativeMetadata,
    },
    Erc20 {
        contract_address: RpcAddress,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: RpcAddress,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: RpcAddress,
        token_id: U256,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum AllowanceAsset {
    Erc20 {
        contract_address: RpcAddress,
        raw_amount_before: U256,
        raw_amount_after: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum TokenApprovalAsset {
    Erc721 {
        contract_address: RpcAddress,
        token_id: U256,
        approved_address_before: Option<RpcAddress>,
        approved_address_after: Option<RpcAddress>,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum OperatorApprovalAsset {
    Erc721 {
        contract_address: RpcAddress,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: RpcAddress,
    },
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
    changes: Vec<service_core_space::CoreSpaceChange>,
    network: Network,
) -> Result<Vec<Change>, ResponseMappingError> {
    changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| try_map_change(change, network, &format!("changes[{index}]")))
        .collect()
}

fn try_map_change(
    change: service_core_space::CoreSpaceChange,
    network: Network,
    field: &str,
) -> Result<Change, ResponseMappingError> {
    use service_core_space::CoreSpaceChange as Source;

    Ok(match change {
        Source::Asset(change) => try_map_asset_change(change, network, field)?,
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
        Source::NativeBurn {
            from,
            raw_amount,
            metadata,
        } => Change::Burn {
            asset: BurnAsset::Native {
                raw_amount: u256_to_wire(raw_amount),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
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

fn try_map_asset_change(
    change: service_core_space::Change,
    network: Network,
    field: &str,
) -> Result<Change, ResponseMappingError> {
    use service_core_space::Change as Source;

    Ok(match change {
        Source::NativeTransfer {
            from,
            to,
            raw_amount,
            metadata,
        } => Change::Transfer {
            asset: TransferAsset::Native {
                raw_amount: u256_to_wire(raw_amount),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc20Transfer {
            contract_address,
            from,
            to,
            raw_amount,
            metadata,
        } => Change::Transfer {
            asset: TransferAsset::Erc20 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                raw_amount: u256_to_wire(raw_amount),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc20Mint {
            contract_address,
            to,
            raw_amount,
            metadata,
        } => Change::Mint {
            asset: MintAsset::Erc20 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                raw_amount: u256_to_wire(raw_amount),
                metadata: metadata.into(),
            },
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc20Burn {
            contract_address,
            from,
            raw_amount,
            metadata,
        } => Change::Burn {
            asset: BurnAsset::Erc20 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                raw_amount: u256_to_wire(raw_amount),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
        },
        Source::Erc721Transfer {
            contract_address,
            from,
            to,
            token_id,
            metadata,
        } => Change::Transfer {
            asset: TransferAsset::Erc721 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc721Mint {
            contract_address,
            to,
            token_id,
            metadata,
        } => Change::Mint {
            asset: MintAsset::Erc721 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                metadata: metadata.into(),
            },
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc721Burn {
            contract_address,
            from,
            token_id,
            metadata,
        } => Change::Burn {
            asset: BurnAsset::Erc721 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                metadata: metadata.into(),
            },
            from: map_address(from, network, field, "from")?,
        },
        Source::Erc1155Transfer {
            contract_address,
            from,
            to,
            token_id,
            raw_amount,
        } => Change::Transfer {
            asset: TransferAsset::Erc1155 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                raw_amount: u256_to_wire(raw_amount),
            },
            from: map_address(from, network, field, "from")?,
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc1155Mint {
            contract_address,
            to,
            token_id,
            raw_amount,
        } => Change::Mint {
            asset: MintAsset::Erc1155 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                raw_amount: u256_to_wire(raw_amount),
            },
            to: map_address(to, network, field, "to")?,
        },
        Source::Erc1155Burn {
            contract_address,
            from,
            token_id,
            raw_amount,
        } => Change::Burn {
            asset: BurnAsset::Erc1155 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                raw_amount: u256_to_wire(raw_amount),
            },
            from: map_address(from, network, field, "from")?,
        },
        Source::Erc20Allowance {
            contract_address,
            owner,
            spender,
            raw_amount_before,
            raw_amount_after,
            metadata,
        } => Change::Allowance {
            asset: AllowanceAsset::Erc20 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                raw_amount_before: u256_to_wire(raw_amount_before),
                raw_amount_after: u256_to_wire(raw_amount_after),
                metadata: metadata.into(),
            },
            owner: map_address(owner, network, field, "owner")?,
            spender: map_address(spender, network, field, "spender")?,
        },
        Source::Erc721TokenApproval {
            contract_address,
            token_id,
            approved_address_before,
            approved_address_after,
            metadata,
        } => Change::TokenApproval {
            asset: TokenApprovalAsset::Erc721 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                token_id: u256_to_wire(token_id),
                approved_address_before: try_map_optional_address(
                    approved_address_before,
                    network,
                    field,
                    "approvedAddressBefore",
                )?,
                approved_address_after: try_map_optional_address(
                    approved_address_after,
                    network,
                    field,
                    "approvedAddressAfter",
                )?,
                metadata: metadata.into(),
            },
        },
        Source::Erc721OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
            metadata,
        } => Change::OperatorApproval {
            asset: OperatorApprovalAsset::Erc721 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
                metadata: metadata.into(),
            },
            owner: map_address(owner, network, field, "owner")?,
            operator: map_address(operator, network, field, "operator")?,
            approved_before,
            approved_after,
        },
        Source::Erc1155OperatorApproval {
            contract_address,
            owner,
            operator,
            approved_before,
            approved_after,
        } => Change::OperatorApproval {
            asset: OperatorApprovalAsset::Erc1155 {
                contract_address: map_address(contract_address, network, field, "contractAddress")?,
            },
            owner: map_address(owner, network, field, "owner")?,
            operator: map_address(operator, network, field, "operator")?,
            approved_before,
            approved_after,
        },
    })
}

fn try_map_sponsorship_configuration(
    configuration: service_core_space::SponsorshipConfiguration,
    network: Network,
    field: &str,
) -> Result<SponsorshipConfiguration, ResponseMappingError> {
    Ok(match configuration {
        service_core_space::SponsorshipConfiguration::Gas {
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
        service_core_space::SponsorshipConfiguration::StorageCollateral {
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
    target: service_core_space::SponsorshipEligibilityTarget,
    network: Network,
    field: &str,
) -> Result<SponsorshipEligibilityTarget, ResponseMappingError> {
    Ok(match target {
        service_core_space::SponsorshipEligibilityTarget::Account(address) => {
            SponsorshipEligibilityTarget::Account {
                address: map_address(address, network, field, "appliesTo.address")?,
            }
        }
        service_core_space::SponsorshipEligibilityTarget::AllAccounts => {
            SponsorshipEligibilityTarget::AllAccounts
        }
    })
}

fn try_map_cross_space_address(
    address: service_core_space::CrossSpaceAddress,
    network: Network,
    field: &str,
    endpoint: &str,
) -> Result<CrossSpaceAddress, ResponseMappingError> {
    Ok(match address {
        service_core_space::CrossSpaceAddress::CoreSpace(address) => CrossSpaceAddress::CoreSpace {
            address: map_address(address, network, field, &format!("{endpoint}.address"))?,
        },
        service_core_space::CrossSpaceAddress::Espace(address) => {
            CrossSpaceAddress::Espace { address }
        }
    })
}

fn try_map_optional_address(
    address: Option<Address>,
    network: Network,
    field: &str,
    name: &str,
) -> Result<Option<RpcAddress>, ResponseMappingError> {
    address
        .map(|address| map_address(address, network, field, name))
        .transpose()
}

fn map_address(
    address: Address,
    network: Network,
    field: &str,
    name: &str,
) -> Result<RpcAddress, ResponseMappingError> {
    map_core_space_address(
        cfx_types::Address::from_slice(address.as_slice()),
        network,
        format!("{field}.{name}"),
    )
}

impl From<service_core_space::SponsoredResource> for SponsoredResource {
    fn from(resource: service_core_space::SponsoredResource) -> Self {
        match resource {
            service_core_space::SponsoredResource::Gas => Self::Gas,
            service_core_space::SponsoredResource::StorageCollateral => Self::StorageCollateral,
        }
    }
}
