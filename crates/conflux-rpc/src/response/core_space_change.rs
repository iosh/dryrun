use alloy_primitives::Address;
use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_simulation::core_space as simulation_core_space;
use serde::Serialize;

use super::{
    b256_to_wire,
    change::{
        Change as EspaceChange, Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem,
    },
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
        principal_raw_amount: U256,
        reward_raw_amount: U256,
    },
    StakingVoteLock {
        account: RpcAddress,
        required_locked_raw_amount: U256,
        unlock_block_number: U64,
    },
    PosRegistration {
        account: RpcAddress,
        identifier: H256,
        bls_public_key: CoreSpaceRpcBytes,
        vrf_public_key: CoreSpaceRpcBytes,
        initial_vote_count: U64,
        locked_raw_amount: U256,
    },
    PosStakeIncrease {
        account: RpcAddress,
        identifier: H256,
        added_vote_count: U64,
        added_locked_raw_amount: U256,
    },
    PosRetirementRequest {
        account: RpcAddress,
        identifier: H256,
        requested_vote_count: U64,
    },
    GovernanceVoteCast {
        voter: RpcAddress,
        round: U64,
        votes: Vec<GovernanceVote>,
    },
    SponsorshipFunding {
        contract_address: RpcAddress,
        sponsor: RpcAddress,
        contributed_raw_amount: U256,
        pool_credited_raw_amount: U256,
        #[serde(flatten)]
        terms: SponsorshipFundingTerms,
        replacement: Option<SponsorshipReplacement>,
    },
    ContractAdminSet {
        contract_address: RpcAddress,
        admin: Option<RpcAddress>,
    },
    SponsorshipAccessRuleSet {
        contract_address: RpcAddress,
        scope: SponsorshipAccessRuleScope,
        enabled: bool,
    },
    StoragePointConversion {
        contract_address: RpcAddress,
        from_sponsor_pool_raw_amount: U256,
        from_storage_collateral_raw_amount: U256,
    },
    CrossSpaceNativeTransfer {
        from: CrossSpaceAddress,
        to: CrossSpaceAddress,
        raw_amount: U256,
    },
    Espace {
        change: EspaceChange,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeCurrency {
    name: String,
    symbol: String,
    decimals: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "sponsoredResource",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum SponsorshipFundingTerms {
    Gas {
        gas_fee_upper_bound_raw_amount: U256,
    },
    StorageCollateral,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged, rename_all_fields = "camelCase")]
pub(super) enum SponsorshipReplacement {
    Gas {
        previous_sponsor: RpcAddress,
        pool_refunded_raw_amount: U256,
    },
    StorageCollateral {
        previous_sponsor: RpcAddress,
        pool_refunded_raw_amount: U256,
        collateral_compensation_raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct GovernanceVote {
    parameter: GovernanceParameter,
    allocation: VoteAllocation,
    replaced_allocation: Option<VoteAllocation>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum GovernanceParameter {
    PowBaseReward,
    PosRewardInterestRate,
    StoragePointProportion,
    BaseFeeShareProportion,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct VoteAllocation {
    unchanged: U256,
    increase: U256,
    decrease: U256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum SponsorshipAccessRuleScope {
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
        Source::StakingDeposit { account, amount } => Change::StakingDeposit {
            account: map_address(account, network, field, "account")?,
            raw_amount: u256_to_wire(amount),
        },
        Source::StakingWithdrawal {
            account,
            principal_amount,
            reward_amount,
        } => Change::StakingWithdrawal {
            account: map_address(account, network, field, "account")?,
            principal_raw_amount: u256_to_wire(principal_amount),
            reward_raw_amount: u256_to_wire(reward_amount),
        },
        Source::StakingVoteLock {
            account,
            required_locked_amount,
            unlock_block_number,
        } => Change::StakingVoteLock {
            account: map_address(account, network, field, "account")?,
            required_locked_raw_amount: u256_to_wire(required_locked_amount),
            unlock_block_number: unlock_block_number.into(),
        },
        Source::PoSRegistration {
            account,
            identifier,
            bls_public_key,
            vrf_public_key,
            initial_vote_count,
            locked_amount,
        } => Change::PosRegistration {
            account: map_address(account, network, field, "account")?,
            identifier: b256_to_wire(identifier),
            bls_public_key: CoreSpaceRpcBytes::from(bls_public_key.to_vec()),
            vrf_public_key: CoreSpaceRpcBytes::from(vrf_public_key.to_vec()),
            initial_vote_count: initial_vote_count.into(),
            locked_raw_amount: u256_to_wire(locked_amount),
        },
        Source::PoSStakeIncrease {
            account,
            identifier,
            added_vote_count,
            added_locked_amount,
        } => Change::PosStakeIncrease {
            account: map_address(account, network, field, "account")?,
            identifier: b256_to_wire(identifier),
            added_vote_count: added_vote_count.into(),
            added_locked_raw_amount: u256_to_wire(added_locked_amount),
        },
        Source::PoSRetirementRequest {
            account,
            identifier,
            requested_vote_count,
        } => Change::PosRetirementRequest {
            account: map_address(account, network, field, "account")?,
            identifier: b256_to_wire(identifier),
            requested_vote_count: requested_vote_count.into(),
        },
        Source::GovernanceVoteCast {
            voter,
            round,
            votes,
        } => Change::GovernanceVoteCast {
            voter: map_address(voter, network, field, "voter")?,
            round: round.into(),
            votes: votes.into_iter().map(Into::into).collect(),
        },
        Source::SponsorshipFunding {
            resource: _,
            contract_address,
            sponsor,
            contributed_amount,
            pool_credited_amount,
            terms,
            replacement,
        } => Change::SponsorshipFunding {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            sponsor: map_address(sponsor, network, field, "sponsor")?,
            contributed_raw_amount: u256_to_wire(contributed_amount),
            pool_credited_raw_amount: u256_to_wire(pool_credited_amount),
            terms: map_sponsorship_funding_terms(terms),
            replacement: replacement
                .map(|replacement| {
                    Ok(match replacement {
                        simulation_core_space::SponsorshipReplacement::Gas {
                            previous_sponsor,
                            pool_refunded_amount,
                        } => SponsorshipReplacement::Gas {
                            previous_sponsor: map_address(
                                previous_sponsor,
                                network,
                                field,
                                "replacement.previousSponsor",
                            )?,
                            pool_refunded_raw_amount: u256_to_wire(pool_refunded_amount),
                        },
                        simulation_core_space::SponsorshipReplacement::StorageCollateral {
                            previous_sponsor,
                            pool_refunded_amount,
                            collateral_compensation_amount,
                        } => SponsorshipReplacement::StorageCollateral {
                            previous_sponsor: map_address(
                                previous_sponsor,
                                network,
                                field,
                                "replacement.previousSponsor",
                            )?,
                            pool_refunded_raw_amount: u256_to_wire(pool_refunded_amount),
                            collateral_compensation_raw_amount: u256_to_wire(
                                collateral_compensation_amount,
                            ),
                        },
                    })
                })
                .transpose()?,
        },
        Source::ContractAdminSet {
            contract_address,
            admin,
        } => Change::ContractAdminSet {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            admin: try_map_optional_address(admin, network, field, "admin")?,
        },
        Source::SponsorshipAccessRuleSet {
            contract_address,
            scope,
            enabled,
        } => Change::SponsorshipAccessRuleSet {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            scope: try_map_access_rule_scope(scope, network, field)?,
            enabled,
        },
        Source::StoragePointConversion {
            contract_address,
            from_sponsor_pool_amount,
            from_storage_collateral_amount,
        } => Change::StoragePointConversion {
            contract_address: map_address(contract_address, network, field, "contractAddress")?,
            from_sponsor_pool_raw_amount: u256_to_wire(from_sponsor_pool_amount),
            from_storage_collateral_raw_amount: u256_to_wire(from_storage_collateral_amount),
        },
        Source::CrossSpaceNativeTransfer {
            from,
            to,
            raw_amount,
        } => Change::CrossSpaceNativeTransfer {
            from: try_map_cross_space_address(from, network, field, "from")?,
            to: try_map_cross_space_address(to, network, field, "to")?,
            raw_amount: u256_to_wire(raw_amount),
        },
        Source::Espace(change) => Change::Espace {
            change: change.into(),
        },
    })
}

impl From<simulation_core_space::GovernanceVote> for GovernanceVote {
    fn from(source: simulation_core_space::GovernanceVote) -> Self {
        Self {
            parameter: source.parameter.into(),
            allocation: source.allocation.into(),
            replaced_allocation: source.replaced_allocation.map(Into::into),
        }
    }
}

impl From<simulation_core_space::GovernanceParameter> for GovernanceParameter {
    fn from(source: simulation_core_space::GovernanceParameter) -> Self {
        match source {
            simulation_core_space::GovernanceParameter::PowBaseReward => Self::PowBaseReward,
            simulation_core_space::GovernanceParameter::PosRewardInterestRate => {
                Self::PosRewardInterestRate
            }
            simulation_core_space::GovernanceParameter::StoragePointProportion => {
                Self::StoragePointProportion
            }
            simulation_core_space::GovernanceParameter::BaseFeeShareProportion => {
                Self::BaseFeeShareProportion
            }
        }
    }
}

impl From<simulation_core_space::VoteAllocation> for VoteAllocation {
    fn from(source: simulation_core_space::VoteAllocation) -> Self {
        Self {
            unchanged: u256_to_wire(source.unchanged),
            increase: u256_to_wire(source.increase),
            decrease: u256_to_wire(source.decrease),
        }
    }
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

fn map_sponsorship_funding_terms(
    terms: simulation_core_space::SponsorshipFundingTerms,
) -> SponsorshipFundingTerms {
    match terms {
        simulation_core_space::SponsorshipFundingTerms::Gas {
            gas_fee_upper_bound,
        } => SponsorshipFundingTerms::Gas {
            gas_fee_upper_bound_raw_amount: u256_to_wire(gas_fee_upper_bound),
        },
        simulation_core_space::SponsorshipFundingTerms::StorageCollateral => {
            SponsorshipFundingTerms::StorageCollateral
        }
    }
}

fn try_map_access_rule_scope(
    scope: simulation_core_space::SponsorshipAccessRuleScope,
    network: Network,
    field: &str,
) -> Result<SponsorshipAccessRuleScope, ResponseMappingError> {
    Ok(match scope {
        simulation_core_space::SponsorshipAccessRuleScope::Account(address) => {
            SponsorshipAccessRuleScope::Account {
                address: map_address(address, network, field, "scope.address")?,
            }
        }
        simulation_core_space::SponsorshipAccessRuleScope::AllAccounts => {
            SponsorshipAccessRuleScope::AllAccounts
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
