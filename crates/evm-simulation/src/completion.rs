use crate::{
    EthereumChainSpec, EvmNotReadyError, EvmSimulationError, EvmTransactionCompletionError,
    EvmTransactionRejection, TransactionInput, TxType, TypedTransaction,
};
use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    eips::BlockId,
    network::Ethereum,
    providers::{DynProvider, Provider, layers::BlockIdProvider},
};
use alloy_primitives::{Address, U64, U256};
use simulation_core::{
    completion::{self, Completion, FeeSource, TransactionCompletionSource},
    transaction::{TransactionRef, TransactionRequest},
};

pub(crate) async fn complete_transaction(
    input: TransactionInput,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<
    Completion<TypedTransaction, TransactionRequest, EvmTransactionRejection>,
    EvmSimulationError,
> {
    completion::complete_transaction(
        input,
        &EthereumCompletionSource {
            provider,
            block,
            chain_spec,
        },
    )
    .await
}

struct EthereumCompletionSource<'a> {
    provider: &'a DynProvider<Ethereum>,
    block: &'a Sealed<Header>,
    chain_spec: &'a EthereumChainSpec,
}

impl FeeSource for EthereumCompletionSource<'_> {
    type Error = EvmSimulationError;
    fn base_fee(&self) -> Result<U256, Self::Error> {
        self.block
            .base_fee_per_gas()
            .map(U256::from)
            .ok_or_else(|| {
                EvmTransactionCompletionError::MissingBaseFee {
                    block_number: self.block.number(),
                }
                .into()
            })
    }
    async fn gas_price(&self) -> Result<U256, Self::Error> {
        self.provider
            .get_gas_price()
            .await
            .map(U256::from)
            .map_err(|source| EvmTransactionCompletionError::GasPriceSuggestion { source }.into())
    }
    async fn priority_fee(&self) -> Result<U256, Self::Error> {
        self.provider
            .get_max_priority_fee_per_gas()
            .await
            .map(U256::from)
            .map_err(|source| {
                EvmTransactionCompletionError::PriorityFeeSuggestion { source }.into()
            })
    }
    fn max_fee_overflow(&self) -> Self::Error {
        EvmTransactionCompletionError::MaxFeePerGasOverflow.into()
    }
    fn max_fee_per_gas_limit(&self) -> U256 {
        U256::from(u128::MAX)
    }
}

impl TransactionCompletionSource for EthereumCompletionSource<'_> {
    type Rejection = EvmTransactionRejection;
    fn chain_id(&self) -> u64 {
        self.chain_spec.chain_id()
    }
    fn default_type(&self) -> TxType {
        if self.block.base_fee_per_gas().is_some() {
            TxType::Eip1559
        } else {
            TxType::Legacy
        }
    }
    fn check_input(&self, transaction: TransactionRef<'_>, _: TxType) -> Result<(), Self::Error> {
        let (fixed, cap, priority, blob) = match transaction {
            TransactionInput::Partial(tx) => (
                tx.fees.gas_price,
                tx.fees.max_fee_per_gas,
                tx.fees.max_priority_fee_per_gas,
                tx.max_fee_per_blob_gas,
            ),
            TransactionInput::Complete(tx) => (
                tx.dynamic_fees().is_none().then(|| tx.gas_price_cap()),
                tx.dynamic_fees().map(|fees| fees.max_fee_per_gas),
                tx.dynamic_fees().map(|fees| fees.max_priority_fee_per_gas),
                match tx {
                    TypedTransaction::Eip4844 {
                        max_fee_per_blob_gas,
                        ..
                    } => Some(*max_fee_per_blob_gas),
                    _ => None,
                },
            ),
        };
        for (field, value) in [
            ("gasPrice", fixed),
            ("maxFeePerGas", cap),
            ("maxPriorityFeePerGas", priority),
            ("maxFeePerBlobGas", blob),
        ] {
            if let Some(value) = value {
                crate::transaction::fee_to_u128(field, value)?;
            }
        }
        Ok(())
    }
    fn rejection(
        &self,
        transaction: TransactionRef<'_>,
        transaction_type: TxType,
    ) -> Result<Option<Self::Rejection>, Self::Error> {
        use EvmTransactionRejection as Rejection;
        use revm::primitives::hardfork::SpecId;

        let spec = self
            .chain_spec
            .execution_spec(self.block.number(), self.block.timestamp())
            .map_err(EvmNotReadyError::from)?
            .spec_id;
        if let Some(chain_id) = transaction.chain_id()
            && chain_id != self.chain_spec.chain_id()
        {
            return Ok(Some(Rejection::InvalidChainId {
                transaction_chain_id: chain_id,
                expected_chain_id: self.chain_spec.chain_id(),
            }));
        }
        let inactive = match transaction_type {
            TxType::Eip2930 if !spec.is_enabled_in(SpecId::BERLIN) => {
                Some(Rejection::Eip2930NotActivated)
            }
            TxType::Eip1559 if !spec.is_enabled_in(SpecId::LONDON) => {
                Some(Rejection::Eip1559NotActivated)
            }
            TxType::Eip4844 if !spec.is_enabled_in(SpecId::CANCUN) => {
                Some(Rejection::Eip4844NotActivated)
            }
            TxType::Eip7702 if !spec.is_enabled_in(SpecId::PRAGUE) => {
                Some(Rejection::Eip7702NotActivated)
            }
            _ => None,
        };
        if inactive.is_some() {
            return Ok(inactive);
        }
        if let (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) =
            (transaction.gas_price_cap(), transaction.priority_fee())
            && max_priority_fee_per_gas > max_fee_per_gas
        {
            return Ok(Some(Rejection::PriorityFeeGreaterThanMaxFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            }));
        }
        if let Some(gas_limit) = transaction.gas_limit()
            && gas_limit > self.block.gas_limit()
        {
            return Ok(Some(Rejection::GasLimitExceedsBlockGasLimit {
                gas_limit,
                block_gas_limit: self.block.gas_limit(),
            }));
        }
        if spec.is_enabled_in(SpecId::LONDON)
            && let (Some(gas_price), Some(base_fee_per_gas)) =
                (transaction.gas_price_cap(), self.block.base_fee_per_gas())
            && gas_price < U256::from(base_fee_per_gas)
        {
            return Ok(Some(Rejection::GasPriceBelowBaseFee {
                gas_price,
                base_fee_per_gas,
            }));
        }
        Ok(None)
    }
    async fn nonce(&self, from: Address) -> Result<u64, Self::Error> {
        let anchored = BlockIdProvider::new(
            self.provider.clone(),
            BlockId::hash_canonical(self.block.hash()),
        );
        anchored
            .get_transaction_count(from)
            .await
            .map_err(|source| {
                EvmTransactionCompletionError::NonceLookup {
                    block_number: self.block.number(),
                    source,
                }
                .into()
            })
    }
    async fn estimate_gas(&self, transaction: &TransactionRequest) -> Result<u64, Self::Error> {
        self.provider
            .raw_request::<_, U64>(
                "eth_estimateGas".into(),
                (transaction, BlockId::hash_canonical(self.block.hash())),
            )
            .await
            .map(|value| value.to::<u64>())
            .map_err(|source| {
                EvmTransactionCompletionError::GasEstimation {
                    block_number: self.block.number(),
                    source,
                }
                .into()
            })
    }
    async fn blob_fee(&self) -> Result<U256, Self::Error> {
        let spec = self
            .chain_spec
            .execution_spec(self.block.number(), self.block.timestamp())
            .map_err(EvmNotReadyError::from)?;
        let params = spec
            .blob_params
            .ok_or(EvmTransactionCompletionError::MissingBlobBaseFee {
                block_number: self.block.number(),
            })?;
        let excess = self.block.excess_blob_gas().ok_or(
            EvmTransactionCompletionError::MissingBlobBaseFee {
                block_number: self.block.number(),
            },
        )?;
        Ok(U256::from(params.calc_blob_fee(excess)))
    }
}
