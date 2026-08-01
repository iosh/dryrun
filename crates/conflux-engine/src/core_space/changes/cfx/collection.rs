use alloy_primitives::U256;
use cfx_executor::{executive_observer::AddressPocket, machine::Machine};
use cfx_types::{AddressSpaceUtil, Space};
use cfx_vm_types::{CallType, Spec};
use contract_standards::Position;

use super::{
    CfxBalanceLocation, CfxOperation, CfxOperations,
    sponsorship::{
        collect_sponsorship_call, collect_standalone_sponsorship_refund,
        collect_storage_point_conversion,
    },
};
use crate::{
    ConfluxEngineError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug, Default)]
struct CfxOperationCollector {
    operations: Vec<CfxOperation>,
}

pub(crate) fn collect_cfx_operations(
    observations: &[Observation],
    machine: &Machine,
    spec: &Spec,
) -> Result<CfxOperations, ConfluxEngineError> {
    let mut collector = CfxOperationCollector::default();
    let mut observation_index = 0;

    while observation_index < observations.len() {
        let observation = &observations[observation_index];
        match observation {
            Observation::Call { .. } => {
                if let Some((operation, consumed)) =
                    collect_sponsorship_call(observations, observation_index, machine, spec)?
                {
                    collector
                        .operations
                        .push(CfxOperation::SponsorshipCall(operation));
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

    Ok(collector.into_operations())
}

fn advance_observation_index(
    observation_index: usize,
    consumed: usize,
) -> Result<usize, ConfluxEngineError> {
    observation_index.checked_add(consumed).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(
            "Core Space CFX observation index overflowed during collection",
        )
    })
}

impl CfxOperationCollector {
    fn into_operations(self) -> CfxOperations {
        CfxOperations::from_operations(self.operations)
    }

    fn collect_call_value_transfer(
        &mut self,
        observation: &Observation,
        machine: &Machine,
        spec: &Spec,
    ) -> Result<(), ConfluxEngineError> {
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
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space CFX collector received a non-call observation",
            ));
        };
        let amount = u256_from_cfx(*transferred_value);

        if amount.is_zero() {
            return Ok(());
        }
        if *space != Space::Native {
            return Err(ConfluxEngineError::analysis_failed(
                "nonzero eSpace call value is not supported by Core Space CFX analysis",
            ));
        }
        if *call_type != CallType::Call {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "nonzero Core Space {call_type:?} value is not an ordinary CFX transfer"
            )));
        }
        if machine
            .internal_contracts()
            .contract(&code_address.with_native_space(), spec)
            .is_some()
        {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space internal-contract call value to {target:?} cannot be classified as an ordinary CFX transfer"
            )));
        }

        self.push_account_transfer(
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
    ) -> Result<(), ConfluxEngineError> {
        if amount.is_zero() {
            return Ok(());
        }
        if space != Space::Native {
            return Err(ConfluxEngineError::analysis_failed(
                "nonzero eSpace create value is not supported by Core Space CFX analysis",
            ));
        }

        self.push_account_transfer(
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
    ) -> Result<usize, ConfluxEngineError> {
        let Some(Observation::InternalTransfer {
            position,
            space,
            from,
            to,
            value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxEngineError::analysis_failed(
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
                ConfluxEngineError::analysis_failed(
                    "Core Space CFX collector received an inconsistent observation index",
                )
            })?,
        )? {
            self.operations
                .push(CfxOperation::SponsorshipStandaloneRefund(refund));
            return Ok(1);
        }

        let amount = u256_from_cfx(*value);

        if matches!(
            (from, to),
            (AddressPocket::StakingBalance(_), AddressPocket::Balance(_))
        ) {
            return self.collect_staking_withdrawal(observations, observation_index);
        }

        if amount.is_zero() {
            return Ok(1);
        }
        if *space != Space::Native {
            return Err(unsupported_internal_transfer(from, to));
        }

        match (from, to) {
            (AddressPocket::Balance(from), AddressPocket::Balance(to))
                if from.space == Space::Native && to.space == Space::Native =>
            {
                self.push_account_transfer(
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
                    payer: CfxBalanceLocation::Account {
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
                    recipient: CfxBalanceLocation::Account {
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
                return Err(ConfluxEngineError::analysis_failed(format!(
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
                return Err(ConfluxEngineError::analysis_failed(
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
    ) -> Result<usize, ConfluxEngineError> {
        let Some(Observation::InternalTransfer {
            position,
            space: withdrawal_space,
            from: AddressPocket::StakingBalance(staking_account),
            to: AddressPocket::Balance(withdrawal_destination),
            value: principal_value,
        }) = observations.get(observation_index)
        else {
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space CFX collector received a non-withdrawal observation",
            ));
        };
        let principal_amount = u256_from_cfx(*principal_value);

        if *withdrawal_space != Space::Native
            || withdrawal_destination.space != Space::Native
            || withdrawal_destination.address != *staking_account
        {
            return Err(ConfluxEngineError::analysis_failed(format!(
                "Core Space staking withdrawal moved CFX between incompatible accounts: {staking_account:?} -> {withdrawal_destination:?}"
            )));
        }

        let expected_reward_position = position.checked_add(1).ok_or_else(|| {
            ConfluxEngineError::analysis_failed(
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
            return Err(ConfluxEngineError::analysis_failed(
                "Core Space staking withdrawal is missing its issuance record",
            ));
        };
        if *reward_position != expected_reward_position
            || *reward_space != Space::Native
            || reward_recipient.space != Space::Native
            || reward_recipient.address != *staking_account
        {
            return Err(ConfluxEngineError::analysis_failed(
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

    fn push_account_transfer(
        &mut self,
        position: usize,
        from: alloy_primitives::Address,
        to: alloy_primitives::Address,
        amount: U256,
    ) {
        if amount.is_zero() {
            return;
        }

        self.operations.push(CfxOperation::AccountTransfer {
            position: Position::new(position, 0),
            from,
            to,
            amount,
        });
    }
}

fn unsupported_internal_transfer(from: &AddressPocket, to: &AddressPocket) -> ConfluxEngineError {
    ConfluxEngineError::analysis_failed(format!(
        "Core Space CFX movement used unsupported {} ({}) -> {} ({}) pockets",
        from.pocket(),
        from.space(),
        to.pocket(),
        to.space()
    ))
}
