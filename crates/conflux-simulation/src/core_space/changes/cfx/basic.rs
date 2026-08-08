use std::collections::BTreeMap;

use alloy_primitives::U256;
use cfx_executor::executive_observer::AddressPocket;
use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_types::{AddressSpaceUtil, Space, address_util::AddressUtil};
use cfx_vm_types::{CallType, Spec};
use contract_standards::legacy::Position;
use primitives::receipt::StorageChange;

use super::{BasicCfxOperation, CfxBalanceLocation, StorageCollateralReleaseOperation};
use crate::{
    ConfluxSimulationError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug)]
pub(super) struct CoreSpaceOperationCollector {
    pending_storage_releases: BTreeMap<cfx_types::Address, U256>,
}

impl CoreSpaceOperationCollector {
    pub(super) fn new(storage_released: &[StorageChange]) -> Result<Self, ConfluxSimulationError> {
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
            pending_storage_releases,
        })
    }

    pub(super) fn collect_call_value_transfer(
        &self,
        observation: &Observation,
        machine: &cfx_executor::machine::Machine,
        spec: &Spec,
    ) -> Result<Option<BasicCfxOperation>, ConfluxSimulationError> {
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
                "Core Space operation collector received a non-call observation",
            ));
        };
        let amount = u256_from_cfx(*transferred_value);

        if amount.is_zero() {
            return Ok(None);
        }
        if *space == Space::Ethereum {
            if *call_type != CallType::Call {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "nonzero eSpace {call_type:?} value is not a balance transfer"
                )));
            }
            return Ok(Some(BasicCfxOperation::EspaceBalanceTransfer {
                from: address_from_cfx(*caller),
                to: address_from_cfx(*target),
                amount,
            }));
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

        Ok(Some(BasicCfxOperation::CoreSpaceBalanceTransfer {
            position: Position::new(*position, 0),
            from: address_from_cfx(*caller),
            to: address_from_cfx(*target),
            amount,
        }))
    }

    pub(super) fn collect_create_value_transfer(
        &self,
        position: usize,
        space: Space,
        from: cfx_types::Address,
        to: cfx_types::Address,
        amount: U256,
    ) -> Option<BasicCfxOperation> {
        if amount.is_zero() {
            return None;
        }
        if space == Space::Ethereum {
            return Some(BasicCfxOperation::EspaceBalanceTransfer {
                from: address_from_cfx(from),
                to: address_from_cfx(to),
                amount,
            });
        }

        Some(BasicCfxOperation::CoreSpaceBalanceTransfer {
            position: Position::new(position, 0),
            from: address_from_cfx(from),
            to: address_from_cfx(to),
            amount,
        })
    }

    pub(super) fn collect_internal_transfer(
        &mut self,
        observations: &[Observation],
        observation_index: usize,
    ) -> Result<(Option<BasicCfxOperation>, usize), ConfluxSimulationError> {
        let Some(Observation::InternalTransfer {
            position,
            space,
            from,
            to,
            value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space operation collector received an inconsistent observation index",
            ));
        };

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
            return Ok((
                Some(BasicCfxOperation::StorageCollateralRelease(
                    StorageCollateralReleaseOperation {
                        contract_address: address_from_cfx(*collateral_contract),
                        total_released_amount,
                        observed_non_point_amount: amount,
                    },
                )),
                1,
            ));
        }

        if matches!(
            (from, to),
            (AddressPocket::StakingBalance(_), AddressPocket::Balance(_))
        ) {
            return self.collect_staking_withdrawal(observations, observation_index);
        }

        if amount.is_zero() {
            return Ok((None, 1));
        }

        if *space == Space::Ethereum {
            return match (from, to) {
                (AddressPocket::Balance(from), AddressPocket::Balance(to))
                    if from.space == Space::Ethereum && to.space == Space::Ethereum =>
                {
                    Ok((
                        Some(BasicCfxOperation::EspaceBalanceTransfer {
                            from: address_from_cfx(from.address),
                            to: address_from_cfx(to.address),
                            amount,
                        }),
                        1,
                    ))
                }
                _ => Err(unsupported_internal_transfer(from, to)),
            };
        }

        if *space != Space::Native {
            return Err(unsupported_internal_transfer(from, to));
        }

        let operation = match (from, to) {
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if from.space == Space::Native && to.space == Space::Native =>
            {
                BasicCfxOperation::CoreSpaceBalanceTransfer {
                    position: Position::new(*position, 0),
                    from: address_from_cfx(from.address),
                    to: address_from_cfx(to.address),
                    amount,
                }
            }
            (AddressPocket::Balance(payer), AddressPocket::GasPayment)
                if payer.space == Space::Native =>
            {
                BasicCfxOperation::GasPrecharge {
                    payer: CfxBalanceLocation::CoreSpaceAccount {
                        account: address_from_cfx(payer.address),
                    },
                    amount,
                }
            }
            (AddressPocket::SponsorBalanceForGas(contract_address), AddressPocket::GasPayment) => {
                BasicCfxOperation::GasPrecharge {
                    payer: CfxBalanceLocation::GasSponsor {
                        contract_address: address_from_cfx(*contract_address),
                    },
                    amount,
                }
            }
            (AddressPocket::GasPayment, AddressPocket::Balance(recipient))
                if recipient.space == Space::Native =>
            {
                BasicCfxOperation::GasRefund {
                    recipient: CfxBalanceLocation::CoreSpaceAccount {
                        account: address_from_cfx(recipient.address),
                    },
                    amount,
                }
            }
            (AddressPocket::GasPayment, AddressPocket::SponsorBalanceForGas(contract_address)) => {
                BasicCfxOperation::GasRefund {
                    recipient: CfxBalanceLocation::GasSponsor {
                        contract_address: address_from_cfx(*contract_address),
                    },
                    amount,
                }
            }
            (AddressPocket::Balance(account), AddressPocket::StakingBalance(staking_account))
                if account.space == Space::Native && account.address == *staking_account =>
            {
                BasicCfxOperation::StakingDeposit {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(account.address),
                    amount,
                }
            }
            (AddressPocket::Balance(account), AddressPocket::StakingBalance(staking_account)) => {
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space staking deposit moved CFX between different accounts: {account:?} -> {staking_account:?}"
                )));
            }
            (AddressPocket::Balance(account), AddressPocket::MintBurn)
                if account.space == Space::Native =>
            {
                BasicCfxOperation::NativeBurn {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(account.address),
                    amount,
                }
            }
            (AddressPocket::StakingBalance(account), AddressPocket::MintBurn) => {
                BasicCfxOperation::StakingBurn {
                    position: Position::new(*position, 0),
                    account: address_from_cfx(*account),
                    amount,
                }
            }
            (AddressPocket::MintBurn, AddressPocket::Balance(_)) => {
                return Err(ConfluxSimulationError::analysis_failed(
                    "Core Space issuance was not paired with a staking withdrawal",
                ));
            }
            _ => return Err(unsupported_internal_transfer(from, to)),
        };

        Ok((Some(operation), 1))
    }

    pub(super) fn finish(self) -> Result<Vec<BasicCfxOperation>, ConfluxSimulationError> {
        Ok(self
            .pending_storage_releases
            .into_iter()
            .map(|(contract_address, total_released_amount)| {
                BasicCfxOperation::StorageCollateralRelease(StorageCollateralReleaseOperation {
                    contract_address: address_from_cfx(contract_address),
                    total_released_amount,
                    observed_non_point_amount: U256::ZERO,
                })
            })
            .collect())
    }

    fn collect_staking_withdrawal(
        &mut self,
        observations: &[Observation],
        observation_index: usize,
    ) -> Result<(Option<BasicCfxOperation>, usize), ConfluxSimulationError> {
        let Some(Observation::InternalTransfer {
            position,
            space: withdrawal_space,
            from: AddressPocket::StakingBalance(staking_account),
            to: AddressPocket::Balance(withdrawal_destination),
            value: principal_value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxSimulationError::analysis_failed(
                "Core Space operation collector received a non-withdrawal observation",
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
        let operation = (!principal_amount.is_zero() || !reward_amount.is_zero()).then(|| {
            BasicCfxOperation::StakingWithdrawal {
                position: Position::new(*position, 0),
                account: address_from_cfx(*staking_account),
                principal_amount,
                reward_amount,
            }
        });

        Ok((operation, 2))
    }
}

fn unsupported_internal_transfer(
    from: &AddressPocket,
    to: &AddressPocket,
) -> ConfluxSimulationError {
    ConfluxSimulationError::analysis_failed(format!(
        "Core Space operation collector encountered unsupported {} ({}) -> {} ({}) pockets",
        from.pocket(),
        from.space(),
        to.pocket(),
        to.space()
    ))
}
