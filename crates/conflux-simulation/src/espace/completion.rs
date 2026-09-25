use super::{
    EspaceContext, EspaceSimulationError, EspaceTransactionCompletionError, EspaceTransactionInput,
    EspaceTransactionRejection, EspaceTransactionRequest, EspaceTypedTransaction, TxType,
};
use crate::{primitive::u256_from_cfx, state::ConfluxSimulationProvider};
use alloy_primitives::{Address, U256};
use simulation_core::completion::{self, Completion, FeeSource, TransactionCompletionSource};

pub(crate) async fn complete_transaction(
    input: EspaceTransactionInput,
    provider: &ConfluxSimulationProvider,
    context: &EspaceContext,
    chain_id: u64,
    rules: crate::chain_spec::EspaceTransactionValidationRules,
) -> Result<
    Completion<EspaceTypedTransaction, EspaceTransactionRequest, EspaceTransactionRejection>,
    EspaceSimulationError,
> {
    completion::complete_transaction(
        input,
        &EspaceCompletionSource {
            provider,
            context,
            chain_id,
            rules,
        },
    )
    .await
}

struct EspaceCompletionSource<'a> {
    provider: &'a ConfluxSimulationProvider,
    context: &'a EspaceContext,
    chain_id: u64,
    rules: crate::chain_spec::EspaceTransactionValidationRules,
}
impl FeeSource for EspaceCompletionSource<'_> {
    type Error = EspaceSimulationError;
    fn base_fee(&self) -> Result<U256, Self::Error> {
        Ok(u256_from_cfx(self.context.base_fee_per_gas()))
    }
    async fn gas_price(&self) -> Result<U256, Self::Error> {
        self.provider
            .eth_gas_price()
            .await
            .map(u256_from_cfx)
            .map_err(|source| {
                EspaceTransactionCompletionError::GasPriceSuggestion { source }.into()
            })
    }
    async fn priority_fee(&self) -> Result<U256, Self::Error> {
        self.provider
            .eth_max_priority_fee_per_gas()
            .await
            .map(u256_from_cfx)
            .map_err(|source| {
                EspaceTransactionCompletionError::PriorityFeeSuggestion { source }.into()
            })
    }
    fn max_fee_overflow(&self) -> Self::Error {
        EspaceTransactionCompletionError::MaxFeePerGasOverflow.into()
    }
}
impl TransactionCompletionSource for EspaceCompletionSource<'_> {
    type Rejection = EspaceTransactionRejection;
    fn chain_id(&self) -> u64 {
        self.chain_id
    }
    fn default_type(&self) -> TxType {
        if !self.context.base_fee_per_gas().is_zero() {
            TxType::Eip1559
        } else {
            TxType::Legacy
        }
    }
    fn check_input(
        &self,
        _: simulation_core::transaction::TransactionRef<'_>,
        transaction_type: TxType,
    ) -> Result<(), Self::Error> {
        if transaction_type == TxType::Eip4844 {
            return Err(
                EspaceTransactionCompletionError::UnsupportedTransactionType { transaction_type }
                    .into(),
            );
        }
        Ok(())
    }
    fn rejection(
        &self,
        transaction: simulation_core::transaction::TransactionRef<'_>,
        transaction_type: TxType,
    ) -> Result<Option<Self::Rejection>, Self::Error> {
        Ok(super::transaction_adapter::reject_transaction(
            transaction,
            transaction_type,
            self.chain_id,
            self.rules,
        ))
    }
    async fn nonce(&self, from: Address) -> Result<u64, Self::Error> {
        self.provider
            .eth_get_transaction_count(from, self.context.state_block())
            .await
            .map_err(|source| {
                EspaceTransactionCompletionError::NonceLookup {
                    block_number: self.context.public_context.number,
                    source,
                }
                .into()
            })
    }
    async fn estimate_gas(
        &self,
        transaction: &EspaceTransactionRequest,
    ) -> Result<u64, Self::Error> {
        let value = self
            .provider
            .eth_estimate_gas(transaction, self.context.state_block())
            .await
            .map_err(|source| EspaceTransactionCompletionError::GasEstimation {
                block_number: self.context.public_context.number,
                source,
            })?;
        u64::try_from(value).map_err(|_| {
            EspaceTransactionCompletionError::GasEstimateOutOfRange {
                block_number: self.context.public_context.number,
                value: u256_from_cfx(value),
            }
            .into()
        })
    }
    async fn blob_fee(&self) -> Result<U256, Self::Error> {
        Err(
            EspaceTransactionCompletionError::UnsupportedTransactionType {
                transaction_type: TxType::Eip4844,
            }
            .into(),
        )
    }
}
