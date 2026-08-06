use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, U256};
use cfx_executor::{executive_observer::AddressPocket, machine::Machine};
use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_types::{AddressSpaceUtil, AddressWithSpace, Space, address_util::AddressUtil};
use cfx_vm_types::{CallType, Spec};
use contract_standards::Position;
use primitives::receipt::StorageChange;

use super::{
    CfxBalanceLocation, CfxOperation, CfxOperations, StorageCollateralReleaseOperation,
    cross_space::collect_cross_space_call,
    sponsorship::{
        CollectedSponsorshipCall, collect_admin_change_attempt, collect_sponsorship_call,
        collect_standalone_sponsorship_refund, collect_storage_point_conversion,
    },
};
use crate::{
    ConfluxSimulationError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug)]
struct CfxOperationCollector {
    operations: Vec<CfxOperation>,
    pending_storage_releases: BTreeMap<cfx_types::Address, U256>,
}

pub(crate) fn collect_cfx_operations(
    observations: &[Observation],
    contracts_created: &[AddressWithSpace],
    storage_released: &[StorageChange],
    machine: &Machine,
    spec: &Spec,
) -> Result<CfxOperations, ConfluxSimulationError> {
    let mut collector = CfxOperationCollector::new(storage_released)?;
    let mut contracts_with_admin_change_attempts = BTreeSet::new();
    for observation in observations {
        let Some(attempt) = collect_admin_change_attempt(observation, machine, spec)? else {
            continue;
        };
        if attempt.is_destroy && !spec.cip131 {
            return Err(ConfluxSimulationError::analysis_failed(
                "pre-CIP-131 Core Space contract destruction may delete sponsorship access-rule entries that public RPC cannot enumerate",
            ));
        }
        contracts_with_admin_change_attempts.insert(attempt.contract_address);
    }
    let mut observation_index = 0;

    while observation_index < observations.len() {
        let observation = &observations[observation_index];
        match observation {
            Observation::Call { .. } => {
                if let Some((operation, consumed)) =
                    collect_cross_space_call(observations, observation_index, machine, spec)?
                {
                    collector
                        .operations
                        .push(CfxOperation::CrossSpaceTransfer(operation));
                    observation_index = advance_observation_index(observation_index, consumed)?;
                    continue;
                }
                if let Some((collected_call, consumed)) =
                    collect_sponsorship_call(observations, observation_index, machine, spec)?
                {
                    match collected_call {
                        CollectedSponsorshipCall::Funding(operation) => collector
                            .operations
                            .push(CfxOperation::SponsorshipFunding(*operation)),
                        CollectedSponsorshipCall::AccessRuleUpdates(updates) => collector
                            .operations
                            .extend(updates.into_iter().map(CfxOperation::SponsorshipAccessRule)),
                    }
                    observation_index = advance_observation_index(observation_index, consumed)?;
                    continue;
                }
                collector.collect_call_value_transfer(observation, machine, spec)?;
                observation_index = advance_observation_index(observation_index, 1)?;
            }
            Observation::CreateTransfer {
                position,
                space,
                from,
                to,
                value,
            } => {
                collector.collect_create_value_transfer(
                    *position,
                    *space,
                    *from,
                    *to,
                    u256_from_cfx(*value),
                )?;
                observation_index = advance_observation_index(observation_index, 1)?;
            }
            Observation::InternalTransfer { .. } => {
                let consumed =
                    collector.collect_internal_transfer(observations, observation_index)?;
                observation_index = advance_observation_index(observation_index, consumed)?;
            }
            Observation::Log { .. } => {
                observation_index = advance_observation_index(observation_index, 1)?;
            }
        }
    }

    collector.validate_sponsorship_access_admin_context(
        &contracts_with_admin_change_attempts,
        contracts_created,
    )?;
    collector.into_operations()
}

fn advance_observation_index(
    observation_index: usize,
    consumed: usize,
) -> Result<usize, ConfluxSimulationError> {
    observation_index.checked_add(consumed).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(
            "Core Space CFX observation index overflowed during collection",
        )
    })
}

impl CfxOperationCollector {
    fn new(storage_released: &[StorageChange]) -> Result<Self, ConfluxSimulationError> {
        let drip_per_unit = u256_from_cfx(*DRIPS_PER_STORAGE_COLLATERAL_UNIT);
        let mut pending_storage_releases = BTreeMap::new();

        for release in storage_released {
            if !release.address.is_contract_address() {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space storage release for non-contract owner {:?} is not supported",
                    release.address
                )));
            }
            let total_released_amount = U256::from(release.collaterals.as_u64())
                .checked_mul(drip_per_unit)
                .ok_or_else(|| {
                    ConfluxSimulationError::analysis_failed(format!(
                        "Core Space storage release amount overflowed for contract {:?}",
                        release.address
                    ))
                })?;
            if total_released_amount.is_zero() {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space execution reported a zero storage release for contract {:?}",
                    release.address
                )));
            }
            if pending_storage_releases
                .insert(release.address, total_released_amount)
                .is_some()
            {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space execution reported duplicate storage releases for contract {:?}",
                    release.address
                )));
            }
        }

        Ok(Self {
            operations: Vec::new(),
            pending_storage_releases,
        })
    }

    fn into_operations(mut self) -> Result<CfxOperations, ConfluxSimulationError> {
        for (contract_address, total_released_amount) in self.pending_storage_releases {
            self.operations.push(CfxOperation::StorageCollateralRelease(
                StorageCollateralReleaseOperation {
                    contract_address: address_from_cfx(contract_address),
                    total_released_amount,
                    observed_non_point_amount: U256::ZERO,
                },
            ));
        }
        Ok(CfxOperations::from_operations(self.operations))
    }

    fn validate_sponsorship_access_admin_context(
        &self,
        contracts_with_admin_change_attempts: &BTreeSet<Address>,
        contracts_created: &[AddressWithSpace],
    ) -> Result<(), ConfluxSimulationError> {
        let created_native_contracts: BTreeSet<_> = contracts_created
            .iter()
            .filter(|created| created.space == Space::Native)
            .map(|created| address_from_cfx(created.address))
            .collect();
        for operation in &self.operations {
            let CfxOperation::SponsorshipAccessRule(update) = operation else {
                continue;
            };
            if update.caller_role != super::SponsorshipAccessCallerRole::ContractAdmin {
                continue;
            }
            if contracts_with_admin_change_attempts.contains(&update.contract_address)
                || created_native_contracts.contains(&update.contract_address)
            {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space sponsorship access-rule admin was not stable during the transaction for contract {}",
                    update.contract_address
                )));
            }
        }
        Ok(())
    }

    fn collect_call_value_transfer(
        &mut self,
        observation: &Observation,
        machine: &Machine,
        spec: &Spec,
    ) -> Result<(), ConfluxSimulationError> {
        let Observation::Call {
            position,
            space,
            call_type,
            caller,
            target,
            code_address,
            transferred_value,
            ..
        } = observation
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space CFX collector received a non-call observation",
            ));
        };
        let amount = u256_from_cfx(*transferred_value);

        if amount.is_zero() {
            return Ok(());
        }
        if *space == Space::Ethereum {
            if *call_type != CallType::Call {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "nonzero eSpace {call_type:?} value is not a balance transfer"
                )));
            }
            self.push_espace_balance_transfer(
                address_from_cfx(*caller),
                address_from_cfx(*target),
                amount,
            );
            return Ok(());
        }
        if *call_type != CallType::Call {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "nonzero Core Space {call_type:?} value is not an ordinary CFX transfer"
            )));
        }
        if machine
            .internal_contracts()
            .contract(&code_address.with_native_space(), spec)
            .is_some()
        {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space internal-contract call value to {target:?} cannot be classified as an ordinary CFX transfer"
            )));
        }

        self.push_core_space_balance_transfer(
            *position,
            address_from_cfx(*caller),
            address_from_cfx(*target),
            amount,
        );
        Ok(())
    }

    fn collect_create_value_transfer(
        &mut self,
        position: usize,
        space: Space,
        from: cfx_types::Address,
        to: cfx_types::Address,
        amount: U256,
    ) -> Result<(), ConfluxSimulationError> {
        if amount.is_zero() {
            return Ok(());
        }
        if space == Space::Ethereum {
            self.push_espace_balance_transfer(address_from_cfx(from), address_from_cfx(to), amount);
            return Ok(());
        }

        self.push_core_space_balance_transfer(
            position,
            address_from_cfx(from),
            address_from_cfx(to),
            amount,
        );
        Ok(())
    }

    fn collect_internal_transfer(
        &mut self,
        observations: &[Observation],
        observation_index: usize,
    ) -> Result<usize, ConfluxSimulationError> {
        let Some(Observation::InternalTransfer {
            position,
            space,
            from,
            to,
            value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space CFX collector received an inconsistent observation index",
            ));
        };
        if let Some((conversion, consumed)) =
            collect_storage_point_conversion(observations, observation_index)?
        {
            self.operations
                .push(CfxOperation::StoragePointConversion(conversion));
            return Ok(consumed);
        }
        if let Some(refund) = collect_standalone_sponsorship_refund(
            observations.get(observation_index).ok_or_else(|| {
                ConfluxSimulationError::analysis_failed(
                    "Core Space CFX collector received an inconsistent observation index",
                )
            })?,
        )? {
            self.operations
                .push(CfxOperation::SponsorshipStandaloneRefund(refund));
            return Ok(1);
        }

        let amount = u256_from_cfx(*value);

        if let (
            AddressPocket::StorageCollateral(collateral_contract),
            AddressPocket::SponsorBalanceForStorage(sponsor_contract),
        ) = (from, to)
        {
            if collateral_contract != sponsor_contract {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space storage release moved collateral between different contracts: {collateral_contract:?} -> {sponsor_contract:?}"
                )));
            }
            let total_released_amount = self
                .pending_storage_releases
                .remove(collateral_contract)
                .ok_or_else(|| {
                    ConfluxSimulationError::analysis_failed(format!(
                        "Core Space storage release movement for contract {collateral_contract:?} had no matching execution record"
                    ))
                })?;
            self.operations.push(CfxOperation::StorageCollateralRelease(
                StorageCollateralReleaseOperation {
                    contract_address: address_from_cfx(*collateral_contract),
                    total_released_amount,
                    observed_non_point_amount: amount,
                },
            ));
            return Ok(1);
        }

        if matches!(
            (from, to),
            (AddressPocket::StakingBalance(_), AddressPocket::Balance(_))
        ) {
            return self.collect_staking_withdrawal(observations, observation_index);
        }

        if amount.is_zero() {
            return Ok(1);
        }

        if *space == Space::Ethereum {
            return match (from, to) {
                (AddressPocket::Balance(from), AddressPocket::Balance(to))
                    if from.space == Space::Ethereum && to.space == Space::Ethereum =>
                {
                    self.push_espace_balance_transfer(
                        address_from_cfx(from.address),
                        address_from_cfx(to.address),
                        amount,
                    );
                    Ok(1)
                }
                _ => Err(unsupported_internal_transfer(from, to)),
            };
        }

        if *space != Space::Native {
            return Err(unsupported_internal_transfer(from, to));
        }

        match (from, to) {
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if from.space == Space::Native && to.space == Space::Native =>
            {
                self.push_core_space_balance_transfer(
                    *position,
                    address_from_cfx(from.address),
                    address_from_cfx(to.address),
                    amount,
                );
            }
            (AddressPocket::Balance(payer), AddressPocket::GasPayment)
                if payer.space == Space::Native =>
            {
                self.operations.push(CfxOperation::GasPrecharge {
                    payer: CfxBalanceLocation::CoreSpaceAccount {
                        account: address_from_cfx(payer.address),
                    },
                    amount,
                });
            }
            (AddressPocket::SponsorBalanceForGas(contract_address), AddressPocket::GasPayment) => {
                self.operations.push(CfxOperation::GasPrecharge {
                    payer: CfxBalanceLocation::GasSponsor {
                        contract_address: address_from_cfx(*contract_address),
                    },
                    amount,
                });
            }
            (AddressPocket::GasPayment, AddressPocket::Balance(recipient))
                if recipient.space == Space::Native =>
            {
                self.operations.push(CfxOperation::GasRefund {
                    recipient: CfxBalanceLocation::CoreSpaceAccount {
                        account: address_from_cfx(recipient.address),
                    },
                    amount,
                });
            }
            (AddressPocket::GasPayment, AddressPocket::SponsorBalanceForGas(contract_address)) => {
                self.operations.push(CfxOperation::GasRefund {
                    recipient: CfxBalanceLocation::GasSponsor {
                        contract_address: address_from_cfx(*contract_address),
                    },
                    amount,
                });
            }
            (AddressPocket::Balance(account), AddressPocket::StakingBalance(staking_account))
                if account.space == Space::Native && account.address == *staking_account =>
            {
                self.operations.push(CfxOperation::StakingDeposit {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(account.address),
                    amount,
                });
            }
            (AddressPocket::Balance(account), AddressPocket::StakingBalance(staking_account)) => {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space staking deposit moved CFX between different accounts: {account:?} -> {staking_account:?}"
                )));
            }
            (AddressPocket::Balance(account), AddressPocket::MintBurn)
                if account.space == Space::Native =>
            {
                self.operations.push(CfxOperation::NativeBurn {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(account.address),
                    amount,
                });
            }
            (AddressPocket::StakingBalance(account), AddressPocket::MintBurn) => {
                self.operations.push(CfxOperation::StakingBurn {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(*account),
                    amount,
                });
            }
            (AddressPocket::MintBurn, AddressPocket::Balance(_)) => {
                return Err(ConfluxSimulationError::analysis_failed(
                    "Core Space issuance was not paired with a staking withdrawal",
                ));
            }
            _ => return Err(unsupported_internal_transfer(from, to)),
        }

        Ok(1)
    }

    fn collect_staking_withdrawal(
        &mut self,
        observations: &[Observation],
        observation_index: usize,
    ) -> Result<usize, ConfluxSimulationError> {
        let Some(Observation::InternalTransfer {
            position,
            space: withdrawal_space,
            from: AddressPocket::StakingBalance(staking_account),
            to: AddressPocket::Balance(withdrawal_destination),
            value: principal_value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space CFX collector received a non-withdrawal observation",
            ));
        };
        let principal_amount = u256_from_cfx(*principal_value);

        if *withdrawal_space != Space::Native
            || withdrawal_destination.space != Space::Native
            || withdrawal_destination.address != *staking_account
        {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space staking withdrawal moved CFX between incompatible accounts: {staking_account:?} -> {withdrawal_destination:?}"
            )));
        }

        let expected_reward_position = position.checked_add(1).ok_or_else(|| {
            ConfluxSimulationError::analysis_failed(
                "Core Space staking withdrawal observation position overflowed",
            )
        })?;
        let Some(Observation::InternalTransfer {
            position: reward_position,
            space: reward_space,
            from: AddressPocket::MintBurn,
            to: AddressPocket::Balance(reward_recipient),
            value: reward_value,
        }) = observations.get(observation_index + 1)
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space staking withdrawal is missing its issuance record",
            ));
        };
        if *reward_position != expected_reward_position
            || *reward_space != Space::Native
            || reward_recipient.space != Space::Native
            || reward_recipient.address != *staking_account
        {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space staking withdrawal has an inconsistent issuance record",
            ));
        }

        let reward_amount = u256_from_cfx(*reward_value);
        if !principal_amount.is_zero() || !reward_amount.is_zero() {
            self.operations.push(CfxOperation::StakingWithdrawal {
                position: Position::new(*position, 0),
                account: address_from_cfx(*staking_account),
                principal_amount,
                reward_amount,
            });
        }

        Ok(2)
    }

    fn push_core_space_balance_transfer(
        &mut self,
        position: usize,
        from: alloy_primitives::Address,
        to: alloy_primitives::Address,
        amount: U256,
    ) {
        if amount.is_zero() {
            return;
        }

        self.operations
            .push(CfxOperation::CoreSpaceBalanceTransfer {
                position: Position::new(position, 0),
                from,
                to,
                amount,
            });
    }

    fn push_espace_balance_transfer(
        &mut self,
        from: alloy_primitives::Address,
        to: alloy_primitives::Address,
        amount: U256,
    ) {
        if amount.is_zero() {
            return;
        }

        self.operations
            .push(CfxOperation::EspaceBalanceTransfer { from, to, amount });
    }
}

fn unsupported_internal_transfer(
    from: &AddressPocket,
    to: &AddressPocket,
) -> ConfluxSimulationError {
    ConfluxSimulationError::analysis_failed(format!(
        "Core Space CFX movement used unsupported {} ({}) -> {} ({}) pockets",
        from.pocket(),
        from.space(),
        to.pocket(),
        to.space()
    ))
}
