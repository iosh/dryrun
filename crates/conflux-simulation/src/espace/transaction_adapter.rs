use cfx_types::U256 as CfxU256;
use primitives::transaction::{
    Action, AuthorizationListItem, Eip155Transaction, Eip1559Transaction, Eip2930Transaction,
    Eip7702Transaction, EthereumTransaction,
};

use super::{
    EspaceCompleteTransaction, EspaceCompleteTransactionVariant, EspaceTransactionInputError,
    EspaceTransactionRejection,
};
use crate::{
    chain_spec::EspaceTransactionValidationRules,
    execution::EspaceTransactionInput as ExecutorEspaceTransactionInput,
    primitive::{access_list_to_cfx, address_to_cfx, u256_to_cfx},
};

pub(crate) fn classify_transaction_rejection(
    transaction: &EspaceCompleteTransaction,
    expected_chain_id: u64,
    rules: EspaceTransactionValidationRules,
) -> Option<EspaceTransactionRejection> {
    if transaction.chain_id != expected_chain_id {
        return Some(EspaceTransactionRejection::InvalidChainId {
            transaction_chain_id: transaction.chain_id,
            expected_chain_id,
        });
    }

    let rejection = match &transaction.variant {
        EspaceCompleteTransactionVariant::Legacy { gas_price } => {
            if !rules.legacy_transactions_active {
                Some(EspaceTransactionRejection::LegacyTransactionNotActivated)
            } else if gas_price.is_zero() {
                Some(EspaceTransactionRejection::ZeroGasPrice)
            } else {
                None
            }
        }
        EspaceCompleteTransactionVariant::Eip2930 { gas_price, .. } => {
            if !rules.typed_transactions_active {
                Some(EspaceTransactionRejection::Eip2930NotActivated)
            } else if gas_price.is_zero() {
                Some(EspaceTransactionRejection::ZeroGasPrice)
            } else {
                None
            }
        }
        EspaceCompleteTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => classify_dynamic_fee_rejection(
            *max_fee_per_gas,
            *max_priority_fee_per_gas,
            rules.typed_transactions_active,
            rules.priority_fee_cap_active,
            EspaceTransactionRejection::Eip1559NotActivated,
        ),
        EspaceCompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if !rules.typed_transactions_active || !rules.eip7702_transactions_active {
                Some(EspaceTransactionRejection::Eip7702NotActivated)
            } else {
                classify_dynamic_fee_rejection(
                    *max_fee_per_gas,
                    *max_priority_fee_per_gas,
                    true,
                    rules.priority_fee_cap_active,
                    EspaceTransactionRejection::Eip7702NotActivated,
                )
            }
        }
    };
    if rejection.is_some() {
        return rejection;
    }

    if rules.initcode_size_limit_active
        && transaction.to.is_none()
        && transaction.input.len() > rules.max_initcode_size
    {
        return Some(EspaceTransactionRejection::CreateInitCodeSizeLimit {
            size: transaction.input.len(),
            limit: rules.max_initcode_size,
        });
    }

    if rules.calldata_floor_active {
        let required_gas = alloy_primitives::U256::from(transaction.input.len())
            * alloy_primitives::U256::from(100_u64);
        if alloy_primitives::U256::from(transaction.gas_limit) < required_gas {
            return Some(EspaceTransactionRejection::CalldataGasRequirement {
                required_gas,
                gas_limit: transaction.gas_limit,
            });
        }
    }

    None
}

fn classify_dynamic_fee_rejection(
    max_fee_per_gas: alloy_primitives::U256,
    max_priority_fee_per_gas: alloy_primitives::U256,
    active: bool,
    enforce_priority_cap: bool,
    inactive_rejection: EspaceTransactionRejection,
) -> Option<EspaceTransactionRejection> {
    if !active {
        Some(inactive_rejection)
    } else if max_fee_per_gas.is_zero() {
        Some(EspaceTransactionRejection::ZeroGasPrice)
    } else if enforce_priority_cap && max_priority_fee_per_gas > max_fee_per_gas {
        Some(EspaceTransactionRejection::PriorityFeeGreaterThanMaxFee {
            max_priority_fee_per_gas,
            max_fee_per_gas,
        })
    } else {
        None
    }
}

pub(crate) fn build_executor_transaction(
    transaction: &EspaceCompleteTransaction,
) -> Result<ExecutorEspaceTransactionInput, EspaceTransactionInputError> {
    let sender = address_to_cfx(transaction.from);
    let chain_id = transaction.chain_id as u32;
    let nonce = CfxU256::from(transaction.nonce);
    let gas = CfxU256::from(transaction.gas_limit);
    let value = u256_to_cfx(transaction.value);
    let data = transaction.input.to_vec();
    let action = transaction.to.map_or(Action::Create, |address| {
        Action::Call(address_to_cfx(address))
    });

    let tx = match &transaction.variant {
        EspaceCompleteTransactionVariant::Legacy { gas_price } => {
            EthereumTransaction::Eip155(Eip155Transaction {
                nonce,
                gas_price: u256_to_cfx(*gas_price),
                gas,
                action,
                value,
                chain_id: Some(chain_id),
                data,
            })
        }
        EspaceCompleteTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => EthereumTransaction::Eip2930(Eip2930Transaction {
            chain_id,
            nonce,
            gas_price: u256_to_cfx(*gas_price),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list.clone()),
        }),
        EspaceCompleteTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => EthereumTransaction::Eip1559(Eip1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list.clone()),
        }),
        EspaceCompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
            authorization_list,
        } => EthereumTransaction::Eip7702(Eip7702Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            destination: address_to_cfx(
                transaction
                    .to
                    .ok_or(EspaceTransactionInputError::Eip7702CallDestinationRequired)?,
            ),
            value,
            data,
            access_list: access_list_to_cfx(access_list.clone()),
            authorization_list: authorization_list
                .iter()
                .map(|authorization| {
                    let inner = authorization.inner();
                    AuthorizationListItem {
                        chain_id: u256_to_cfx(inner.chain_id),
                        address: address_to_cfx(inner.address),
                        nonce: inner.nonce,
                        y_parity: authorization.y_parity(),
                        r: u256_to_cfx(authorization.r()),
                        s: u256_to_cfx(authorization.s()),
                    }
                })
                .collect(),
        }),
    };

    Ok(ExecutorEspaceTransactionInput { tx, sender })
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, Bytes, U256};

    use super::classify_transaction_rejection;
    use crate::{
        chain_spec::ConfluxChainSpec,
        espace::{
            EspaceCompleteTransaction, EspaceCompleteTransactionVariant, EspaceTransactionRejection,
        },
    };

    const CIP645_HEIGHT: u64 = 129_680_000;
    const EIP3860_MAX_INITCODE_SIZE: usize = 49_152;

    #[test]
    fn applies_the_priority_fee_cap_at_its_protocol_activation() {
        let chain_spec = ConfluxChainSpec::mainnet();
        let transaction =
            dynamic_fee_transaction(Some(Address::repeat_byte(2)), Bytes::new(), U256::from(3));

        let before_activation =
            chain_spec.espace_transaction_validation_rules(250_000_000, CIP645_HEIGHT - 1);
        assert_eq!(
            classify_transaction_rejection(&transaction, 1030, before_activation),
            None
        );

        let active = chain_spec.espace_transaction_validation_rules(250_000_000, CIP645_HEIGHT);
        assert!(matches!(
            classify_transaction_rejection(&transaction, 1030, active),
            Some(EspaceTransactionRejection::PriorityFeeGreaterThanMaxFee {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            }) if max_priority_fee_per_gas == U256::from(3) && max_fee_per_gas == U256::from(2)
        ));
    }

    #[test]
    fn enforces_the_activated_initcode_size_boundary() {
        let rules = ConfluxChainSpec::mainnet()
            .espace_transaction_validation_rules(250_000_000, CIP645_HEIGHT);
        assert_eq!(rules.max_initcode_size, EIP3860_MAX_INITCODE_SIZE);
        let mut transaction = dynamic_fee_transaction(
            None,
            Bytes::from(vec![0_u8; EIP3860_MAX_INITCODE_SIZE]),
            U256::from(1),
        );
        transaction.gas_limit = 10_000_000;

        assert_eq!(
            classify_transaction_rejection(&transaction, 1030, rules),
            None
        );

        transaction.input = Bytes::from(vec![0_u8; EIP3860_MAX_INITCODE_SIZE + 1]);
        assert!(matches!(
            classify_transaction_rejection(&transaction, 1030, rules),
            Some(EspaceTransactionRejection::CreateInitCodeSizeLimit { size, limit })
                if size == EIP3860_MAX_INITCODE_SIZE + 1
                    && limit == EIP3860_MAX_INITCODE_SIZE
        ));
    }

    fn dynamic_fee_transaction(
        to: Option<Address>,
        input: Bytes,
        max_priority_fee_per_gas: U256,
    ) -> EspaceCompleteTransaction {
        EspaceCompleteTransaction {
            from: Address::repeat_byte(1),
            to,
            nonce: 0,
            gas_limit: 1_000_000,
            value: U256::ZERO,
            input,
            chain_id: 1030,
            variant: EspaceCompleteTransactionVariant::Eip1559 {
                max_fee_per_gas: U256::from(2),
                max_priority_fee_per_gas,
                access_list: Vec::new(),
            },
        }
    }
}
