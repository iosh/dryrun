use alloy_primitives::Log;
use contract_standards::{Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem};
use evm_simulation::{
    EvmAccountDelegationChange, EvmBlockContext, EvmChanges, EvmExecutionOutcome,
    EvmExecutionResult, EvmNativeCurrency, EvmNativeTransferChange, EvmSelfDestructBurnChange,
    EvmSimulation, EvmStandardChange, EvmStateChange, EvmSuccessOutput,
    EvmWrappedNativeDepositChange, EvmWrappedNativeWithdrawalChange,
};

use crate::interface as rpc;

impl From<EvmSimulation> for rpc::EvmSimulateTransactionResponse {
    fn from(simulation: EvmSimulation) -> Self {
        let EvmSimulation {
            context,
            transaction,
            execution,
            changes,
        } = simulation;

        Self {
            state: context.into(),
            transaction,
            outcome: execution.into(),
            changes: changes.into(),
        }
    }
}

impl From<EvmBlockContext> for rpc::EvmState {
    fn from(context: EvmBlockContext) -> Self {
        Self {
            block_number: context.number,
            block_hash: context.hash,
        }
    }
}

impl From<EvmExecutionOutcome> for rpc::Outcome {
    fn from(outcome: EvmExecutionOutcome) -> Self {
        match outcome {
            EvmExecutionOutcome::Success {
                result,
                output,
                logs,
                ..
            } => {
                let accounting = result.into();
                let logs = logs.into_iter().map(Into::into).collect();
                Self::Success(match output {
                    EvmSuccessOutput::Call { return_data } => {
                        rpc::SuccessOutcome::Call(rpc::SuccessCallOutcome {
                            accounting,
                            return_data,
                            logs,
                        })
                    }
                    EvmSuccessOutput::Create {
                        address,
                        runtime_code,
                    } => rpc::SuccessOutcome::Create(rpc::SuccessCreateOutcome {
                        accounting,
                        contract_address: address,
                        runtime_code,
                        logs,
                    }),
                })
            }
            EvmExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            } => Self::Reverted(rpc::RevertedOutcome {
                accounting: result.into(),
                revert_data,
                reason: reason.map(|reason| reason.to_string()),
            }),
            EvmExecutionOutcome::Halted { result, reason } => Self::Failed(rpc::FailedOutcome {
                accounting: result.into(),
                error: reason.to_string(),
            }),
            EvmExecutionOutcome::NotExecuted(rejection) => Self::Rejected {
                error: rejection.to_string(),
            },
        }
    }
}

impl From<EvmExecutionResult> for rpc::ExecutionAccounting {
    fn from(result: EvmExecutionResult) -> Self {
        let fee = result.fee();
        let execution_fee = fee.execution_gas_fee();
        Self {
            gas_used: result.gas().gas_used(),
            effective_gas_price: execution_fee.effective_gas_price(),
            gas_fee: execution_fee.charged_amount(),
            burnt_gas_fee: execution_fee.burnt_amount_if_applicable(),
            blob: fee.blob_gas_fee().map(|blob| rpc::BlobGasAccounting {
                blob_gas_used: blob.gas_used(),
                blob_gas_price: blob.gas_price(),
                blob_gas_fee: blob.charged_amount(),
            }),
        }
    }
}

impl From<Log> for rpc::SimulationLog {
    fn from(log: Log) -> Self {
        Self {
            address: log.address,
            topics: log.data.topics().to_vec(),
            data: log.data.data,
        }
    }
}

impl From<EvmChanges> for rpc::Changes {
    fn from(changes: EvmChanges) -> Self {
        match changes {
            EvmChanges::Complete(changes) => Self::Complete {
                items: changes.into_items().into_iter().map(Into::into).collect(),
            },
            EvmChanges::Unavailable(error) => Self::Unavailable {
                error: error.to_string(),
            },
        }
    }
}

impl From<EvmStateChange> for rpc::StateChange {
    fn from(change: EvmStateChange) -> Self {
        match change {
            EvmStateChange::NativeTransfer(change) => change.into(),
            EvmStateChange::SelfDestructBurn(change) => change.into(),
            EvmStateChange::AccountDelegation(change) => change.into(),
            EvmStateChange::WrappedNativeDeposit(change) => change.into(),
            EvmStateChange::WrappedNativeWithdrawal(change) => change.into(),
            EvmStateChange::Standard(change) => change.into(),
        }
    }
}

impl From<EvmNativeTransferChange> for rpc::StateChange {
    fn from(change: EvmNativeTransferChange) -> Self {
        Self::NativeTransfer {
            from: change.from,
            to: change.to,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EvmSelfDestructBurnChange> for rpc::StateChange {
    fn from(change: EvmSelfDestructBurnChange) -> Self {
        Self::SelfDestructBurn {
            contract_address: change.contract_address,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EvmAccountDelegationChange> for rpc::StateChange {
    fn from(change: EvmAccountDelegationChange) -> Self {
        Self::AccountDelegation {
            account: change.account,
            before: change.before.into(),
            after: change.after.into(),
        }
    }
}

impl From<EvmWrappedNativeDepositChange> for rpc::StateChange {
    fn from(change: EvmWrappedNativeDepositChange) -> Self {
        Self::WrappedNativeDeposit {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EvmWrappedNativeWithdrawalChange> for rpc::StateChange {
    fn from(change: EvmWrappedNativeWithdrawalChange) -> Self {
        Self::WrappedNativeWithdrawal {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EvmStandardChange> for rpc::StateChange {
    fn from(change: EvmStandardChange) -> Self {
        match change {
            EvmStandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata,
            } => Self::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                metadata,
            } => Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                before,
                after,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata,
            } => Self::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata,
            } => Self::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata: metadata.into(),
            },
            EvmStandardChange::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                metadata,
            } => Self::Erc721Approval {
                contract_address,
                owner,
                before,
                after,
                token_id,
                metadata: metadata.into(),
            },
            EvmStandardChange::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            } => Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                before,
                after,
            },
            EvmStandardChange::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            },
            EvmStandardChange::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155MintSingle {
                contract_address,
                operator,
                to,
                token_id,
                raw_amount,
            },
            EvmStandardChange::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            } => Self::Erc1155BurnSingle {
                contract_address,
                operator,
                from,
                token_id,
                raw_amount,
            },
            EvmStandardChange::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
            EvmStandardChange::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items,
            } => Self::Erc1155MintBatch {
                contract_address,
                operator,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
            EvmStandardChange::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items,
            } => Self::Erc1155BurnBatch {
                contract_address,
                operator,
                from,
                items: items.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<Erc20Metadata> for rpc::Erc20Metadata {
    fn from(metadata: Erc20Metadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<Erc721CollectionMetadata> for rpc::Erc721CollectionMetadata {
    fn from(metadata: Erc721CollectionMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
        }
    }
}

impl From<Erc1155TransferItem> for rpc::Erc1155TransferItem {
    fn from(item: Erc1155TransferItem) -> Self {
        Self {
            token_id: item.token_id,
            raw_amount: item.raw_amount,
        }
    }
}

impl From<evm_simulation::EvmAccountDelegation> for rpc::DelegationState {
    fn from(state: evm_simulation::EvmAccountDelegation) -> Self {
        Self {
            delegate: state.delegate,
            nonce: state.nonce,
        }
    }
}

impl From<EvmNativeCurrency> for rpc::NativeCurrency {
    fn from(currency: EvmNativeCurrency) -> Self {
        Self {
            name: currency.name,
            symbol: currency.symbol,
            decimals: currency.decimals,
        }
    }
}
