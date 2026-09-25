use cfx_types::U256 as CfxU256;
use primitives::transaction::{
    Action, AuthorizationListItem, Eip155Transaction, Eip1559Transaction, Eip2930Transaction,
    Eip7702Transaction, EthereumTransaction,
};

use super::{
    EspaceTransactionInputError, EspaceTransactionRejection, EspaceTypedTransaction, TxType,
};
use crate::{
    chain_spec::EspaceTransactionValidationRules,
    execution::EspaceTransactionInput as ExecutorEspaceTransactionInput,
    primitive::{access_list_to_cfx, address_to_cfx, u256_to_cfx},
};

pub(crate) fn reject_transaction(
    transaction: simulation_core::transaction::TransactionRef<'_>,
    transaction_type: TxType,
    expected_chain_id: u64,
    rules: EspaceTransactionValidationRules,
) -> Option<EspaceTransactionRejection> {
    use EspaceTransactionRejection as Rejection;
    if let Some(chain_id) = transaction.chain_id()
        && chain_id != expected_chain_id
    {
        return Some(Rejection::InvalidChainId {
            transaction_chain_id: chain_id,
            expected_chain_id,
        });
    }
    let inactive = match transaction_type {
        TxType::Legacy if !rules.legacy_transactions_active => {
            Some(Rejection::LegacyTransactionNotActivated)
        }
        TxType::Eip2930 if !rules.typed_transactions_active => Some(Rejection::Eip2930NotActivated),
        TxType::Eip1559 if !rules.typed_transactions_active => Some(Rejection::Eip1559NotActivated),
        TxType::Eip7702
            if !rules.typed_transactions_active || !rules.eip7702_transactions_active =>
        {
            Some(Rejection::Eip7702NotActivated)
        }
        _ => None,
    };
    if inactive.is_some() {
        return inactive;
    }
    if transaction
        .gas_price_cap()
        .is_some_and(|price| price.is_zero())
    {
        return Some(Rejection::ZeroGasPrice);
    }
    if rules.priority_fee_cap_active
        && let (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) =
            (transaction.gas_price_cap(), transaction.priority_fee())
        && max_priority_fee_per_gas > max_fee_per_gas
    {
        return Some(Rejection::PriorityFeeGreaterThanMaxFee {
            max_priority_fee_per_gas,
            max_fee_per_gas,
        });
    }
    if rules.initcode_size_limit_active
        && transaction.to().is_none()
        && transaction.input().len() > rules.max_initcode_size
    {
        return Some(Rejection::CreateInitCodeSizeLimit {
            size: transaction.input().len(),
            limit: rules.max_initcode_size,
        });
    }
    if rules.calldata_floor_active
        && let Some(gas_limit) = transaction.gas_limit()
    {
        let required_gas = alloy_primitives::U256::from(transaction.input().len())
            * alloy_primitives::U256::from(100_u64);
        if alloy_primitives::U256::from(gas_limit) < required_gas {
            return Some(Rejection::CalldataGasRequirement {
                required_gas,
                gas_limit,
            });
        }
    }
    None
}

pub(crate) fn build_executor_transaction(
    transaction: &EspaceTypedTransaction,
) -> Result<ExecutorEspaceTransactionInput, EspaceTransactionInputError> {
    let common = transaction.common();
    let sender = address_to_cfx(common.from);
    let chain_id =
        u32::try_from(common.chain_id).map_err(|_| EspaceTransactionInputError::OutOfRange {
            field: "chainId",
            value: alloy_primitives::U256::from(common.chain_id),
            maximum: alloy_primitives::U256::from(u32::MAX),
        })?;
    let nonce = CfxU256::from(common.nonce);
    let gas = CfxU256::from(common.gas_limit);
    let value = u256_to_cfx(common.value);
    let data = common.input.to_vec();
    let action = common.to.map_or(Action::Create, |address| {
        Action::Call(address_to_cfx(address))
    });

    let tx = match transaction {
        EspaceTypedTransaction::Eip4844 { .. } => {
            return Err(EspaceTransactionInputError::UnsupportedType {
                transaction_type: TxType::Eip4844,
            });
        }
        EspaceTypedTransaction::Legacy { gas_price, .. } => {
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
        EspaceTypedTransaction::Eip2930 {
            gas_price,
            access_list,
            ..
        } => EthereumTransaction::Eip2930(Eip2930Transaction {
            chain_id,
            nonce,
            gas_price: u256_to_cfx(*gas_price),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
        EspaceTypedTransaction::Eip1559 {
            fees:
                simulation_core::transaction::DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
            access_list,
            ..
        } => EthereumTransaction::Eip1559(Eip1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
        EspaceTypedTransaction::Eip7702 {
            fees:
                simulation_core::transaction::DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
            access_list,
            authorization_list,
            ..
        } => EthereumTransaction::Eip7702(Eip7702Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            destination: address_to_cfx(common.to.ok_or(
                EspaceTransactionInputError::MissingField {
                    transaction_type: TxType::Eip7702,
                    field: "to",
                },
            )?),
            value,
            data,
            access_list: access_list_to_cfx(access_list),
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

    use super::reject_transaction;
    use crate::{
        chain_spec::ConfluxChainSpec,
        espace::{
            EspaceTransactionCommon, EspaceTransactionRejection, EspaceTypedTransaction, TxType,
        },
    };
    use simulation_core::transaction::TransactionInput;

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
            reject_transaction(
                TransactionInput::Complete(&transaction),
                TxType::Eip1559,
                1030,
                before_activation
            ),
            None
        );

        let active = chain_spec.espace_transaction_validation_rules(250_000_000, CIP645_HEIGHT);
        assert!(matches!(
            reject_transaction(TransactionInput::Complete(&transaction), TxType::Eip1559, 1030, active),
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
        transaction.common_mut().gas_limit = 10_000_000;

        assert_eq!(
            reject_transaction(
                TransactionInput::Complete(&transaction),
                TxType::Eip1559,
                1030,
                rules
            ),
            None
        );

        transaction.common_mut().input = Bytes::from(vec![0_u8; EIP3860_MAX_INITCODE_SIZE + 1]);
        assert!(matches!(
            reject_transaction(TransactionInput::Complete(&transaction), TxType::Eip1559, 1030, rules),
            Some(EspaceTransactionRejection::CreateInitCodeSizeLimit { size, limit })
                if size == EIP3860_MAX_INITCODE_SIZE + 1
                    && limit == EIP3860_MAX_INITCODE_SIZE
        ));
    }

    fn dynamic_fee_transaction(
        to: Option<Address>,
        input: Bytes,
        max_priority_fee_per_gas: U256,
    ) -> EspaceTypedTransaction {
        EspaceTypedTransaction::Eip1559 {
            common: EspaceTransactionCommon {
                from: Address::repeat_byte(1),
                to,
                nonce: 0,
                gas_limit: 1_000_000,
                value: U256::ZERO,
                input,
                chain_id: 1030,
            },
            fees: simulation_core::transaction::DynamicFees {
                max_fee_per_gas: U256::from(2),
                max_priority_fee_per_gas,
            },
            access_list: Vec::new(),
        }
    }
}
