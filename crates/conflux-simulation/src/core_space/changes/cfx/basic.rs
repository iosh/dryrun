use std::collections::BTreeMap;

use crate::core_space::changes::ChangePosition;
use alloy_primitives::U256;
use cfx_executor::executive_observer::AddressPocket;
use cfx_parameters::staking::DRIPS_PER_STORAGE_COLLATERAL_UNIT;
use cfx_types::{AddressSpaceUtil, Space, address_util::AddressUtil};
use cfx_vm_types::{CallType, Spec};
use primitives::receipt::StorageChange;

use super::{BasicCfxOperation, CfxBalanceLocation, StorageCollateralReleaseOperation};
use crate::{
    core_space::CoreSpaceChangesError,
    execution::{CommittedExecutionTrace, FrameAction, FrameId, TraceEvent},
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug)]
pub(super) struct CoreSpaceOperationCollector {
    pending_storage_releases: BTreeMap<cfx_types::Address, U256>,
}

impl CoreSpaceOperationCollector {
    pub(super) fn new(storage_released: &[StorageChange]) -> Result<Self, CoreSpaceChangesError> {
        let drip_per_unit = u256_from_cfx(*DRIPS_PER_STORAGE_COLLATERAL_UNIT);
        let mut pending_storage_releases = BTreeMap::new();

        for release in storage_released {
            if !release.address.is_contract_address() {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space storage release for non-contract owner {:?} is not supported",
                    release.address
                )));
            }
            let total_released_amount = U256::from(release.collaterals.as_u64())
                .checked_mul(drip_per_unit)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space storage release amount overflowed for contract {:?}",
                        release.address
                    ))
                })?;
            if total_released_amount.is_zero() {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space execution reported a zero storage release for contract {:?}",
                    release.address
                )));
            }
            if pending_storage_releases
                .insert(release.address, total_released_amount)
                .is_some()
            {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
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
        trace: &CommittedExecutionTrace,
        position: usize,
        frame_id: FrameId,
        machine: &cfx_executor::machine::Machine,
        spec: &Spec,
    ) -> Result<Option<BasicCfxOperation>, CoreSpaceChangesError> {
        let frame = trace.frame(frame_id);
        let FrameAction::Call {
            call_type,
            caller,
            target,
            code_address,
            transferred_value,
            ..
        } = &frame.action
        else {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space operation collector received a non-call frame",
            ));
        };
        let amount = u256_from_cfx(*transferred_value);

        if amount.is_zero() {
            return Ok(None);
        }
        if frame.space == Space::Ethereum {
            if *call_type != CallType::Call {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "nonzero eSpace {call_type:?} value is not a balance transfer"
                )));
            }
            return Ok(Some(BasicCfxOperation::EspaceBalanceTransfer {
                position: ChangePosition::new(position, 0),
                from: address_from_cfx(*caller),
                to: address_from_cfx(*target),
                amount,
            }));
        }
        if *call_type != CallType::Call {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "nonzero Core Space {call_type:?} value is not an ordinary CFX transfer"
            )));
        }
        if machine
            .internal_contracts()
            .contract(&code_address.with_native_space(), spec)
            .is_some()
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space internal-contract call value to {target:?} cannot be classified as an ordinary CFX transfer"
            )));
        }

        Ok(Some(BasicCfxOperation::CoreSpaceBalanceTransfer {
            position: ChangePosition::new(position, 0),
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
                position: ChangePosition::new(position, 0),
                from: address_from_cfx(from),
                to: address_from_cfx(to),
                amount,
            });
        }

        Some(BasicCfxOperation::CoreSpaceBalanceTransfer {
            position: ChangePosition::new(position, 0),
            from: address_from_cfx(from),
            to: address_from_cfx(to),
            amount,
        })
    }

    pub(super) fn collect_internal_transfer(
        &mut self,
        event: &TraceEvent,
    ) -> Result<(Option<BasicCfxOperation>, Vec<usize>), CoreSpaceChangesError> {
        let TraceEvent::InternalTransfer {
            position,
            frame_id: _,
            space,
            from,
            to,
            value,
        } = event
        else {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space operation collector received a non-movement operation",
            ));
        };

        let amount = u256_from_cfx(*value);

        if let (
            AddressPocket::StorageCollateral(collateral_contract),
            AddressPocket::SponsorBalanceForStorage(sponsor_contract),
        ) = (from, to)
        {
            if collateral_contract != sponsor_contract {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space storage release moved collateral between different contracts: {collateral_contract:?} -> {sponsor_contract:?}"
                )));
            }
            let total_released_amount = self
                .pending_storage_releases
                .remove(collateral_contract)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
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
                vec![*position],
            ));
        }

        if amount.is_zero() {
            return Ok((None, vec![*position]));
        }

        if *space == Space::Ethereum {
            return match (from, to) {
                (AddressPocket::Balance(from), AddressPocket::Balance(to))
                    if from.space == Space::Ethereum && to.space == Space::Ethereum =>
                {
                    Ok((
                        Some(BasicCfxOperation::EspaceBalanceTransfer {
                            position: ChangePosition::new(*position, 0),
                            from: address_from_cfx(from.address),
                            to: address_from_cfx(to.address),
                            amount,
                        }),
                        vec![*position],
                    ))
                }
                (AddressPocket::Balance(account), AddressPocket::MintBurn)
                    if account.space == Space::Ethereum =>
                {
                    Ok((
                        Some(BasicCfxOperation::EspaceNativeBurn {
                            position: ChangePosition::new(*position, 0),
                            account: address_from_cfx(account.address),
                            amount,
                        }),
                        vec![*position],
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
                    position: ChangePosition::new(*position, 0),
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
            (AddressPocket::Balance(_), AddressPocket::StakingBalance(_))
            | (AddressPocket::StakingBalance(_), AddressPocket::Balance(_)) => {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space staking pocket movement was not claimed by a canonical staking operation",
                ));
            }
            (AddressPocket::Balance(account), AddressPocket::MintBurn)
                if account.space == Space::Native =>
            {
                BasicCfxOperation::NativeBurn {
                    position: ChangePosition::new(*position, 0),
                    account: address_from_cfx(account.address),
                    amount,
                }
            }
            (AddressPocket::MintBurn, AddressPocket::Balance(_)) => {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space issuance was not paired with a staking withdrawal",
                ));
            }
            _ => return Err(unsupported_internal_transfer(from, to)),
        };

        Ok((Some(operation), vec![*position]))
    }

    pub(super) fn finish(self) -> Result<Vec<BasicCfxOperation>, CoreSpaceChangesError> {
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
}

fn unsupported_internal_transfer(
    from: &AddressPocket,
    to: &AddressPocket,
) -> CoreSpaceChangesError {
    CoreSpaceChangesError::unsupported_operation(format!(
        "Core Space operation collector encountered unsupported {} ({}) -> {} ({}) pockets",
        from.pocket(),
        from.space(),
        to.pocket(),
        to.space()
    ))
}
