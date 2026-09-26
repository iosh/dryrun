use alloy_primitives::{Address, U256};
use cfx_types::U64;
use conflux_simulation::espace::{
    EspaceAccountDelegation, EspaceAccountDelegationChange, EspaceNativeTransferChange,
    EspaceSelfDestructBurnChange, EspaceSimulation, EspaceStandardChange, EspaceStateChange,
    EspaceWrappedNativeDepositChange, EspaceWrappedNativeWithdrawalChange,
};
use serde::Serialize;

use super::change::{
    Erc20Metadata as RpcErc20Metadata, Erc721CollectionMetadata as RpcErc721CollectionMetadata,
    Erc1155TransferItem as RpcErc1155TransferItem, NativeCurrency,
};

mod u256_hex {
    use alloy_primitives::U256;
    use serde::{Serialize, Serializer};

    pub(super) fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(crate) struct SimulateEspaceTransactionResponse(
    std::sync::Arc<
        simulation_core::simulation::Simulation<
            conflux_simulation::espace::EspaceBlockContext,
            conflux_simulation::espace::EspaceTypedTransaction,
            conflux_simulation::espace::EspaceTransactionRequest,
            conflux_simulation::espace::EspaceExecutionOutcome,
            conflux_simulation::espace::EspaceTransactionRejection,
            Vec<StandaloneChange>,
            conflux_simulation::espace::EspaceAnalysisError,
        >,
    >,
);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StandaloneChange {
    NativeTransfer {
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    AccountDelegation {
        account: Address,
        before: DelegationState,
        after: DelegationState,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc20Approval {
        contract_address: Address,
        owner: Address,
        spender: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        before: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        after: U256,
        #[serde(flatten)]
        metadata: RpcErc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: RpcErc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155MintSingle {
        contract_address: Address,
        operator: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155BurnSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
    Erc1155MintBatch {
        contract_address: Address,
        operator: Address,
        to: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
    Erc1155BurnBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        items: Vec<RpcErc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DelegationState {
    delegate: Option<Address>,
    nonce: U64,
}

impl From<EspaceSimulation> for SimulateEspaceTransactionResponse {
    fn from(simulation: EspaceSimulation) -> Self {
        Self(std::sync::Arc::new(simulation.map_changes(|changes| {
            changes.into_items().into_iter().map(Into::into).collect()
        })))
    }
}

impl From<EspaceStateChange> for StandaloneChange {
    fn from(change: EspaceStateChange) -> Self {
        match change {
            EspaceStateChange::NativeTransfer(change) => change.into(),
            EspaceStateChange::SelfDestructBurn(change) => change.into(),
            EspaceStateChange::WrappedNativeDeposit(change) => change.into(),
            EspaceStateChange::WrappedNativeWithdrawal(change) => change.into(),
            EspaceStateChange::AccountDelegation(change) => change.into(),
            EspaceStateChange::Standard(change) => change.into(),
        }
    }
}

impl From<EspaceNativeTransferChange> for StandaloneChange {
    fn from(change: EspaceNativeTransferChange) -> Self {
        Self::NativeTransfer {
            from: change.from,
            to: change.to,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EspaceSelfDestructBurnChange> for StandaloneChange {
    fn from(change: EspaceSelfDestructBurnChange) -> Self {
        Self::SelfDestructBurn {
            contract_address: change.contract_address,
            raw_amount: change.raw_amount,
            currency: change.currency.into(),
        }
    }
}

impl From<EspaceWrappedNativeDepositChange> for StandaloneChange {
    fn from(change: EspaceWrappedNativeDepositChange) -> Self {
        Self::WrappedNativeDeposit {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EspaceWrappedNativeWithdrawalChange> for StandaloneChange {
    fn from(change: EspaceWrappedNativeWithdrawalChange) -> Self {
        Self::WrappedNativeWithdrawal {
            contract_address: change.contract_address,
            account: change.account,
            raw_amount: change.raw_amount,
            metadata: change.metadata.into(),
        }
    }
}

impl From<EspaceAccountDelegationChange> for StandaloneChange {
    fn from(change: EspaceAccountDelegationChange) -> Self {
        Self::AccountDelegation {
            account: change.account,
            before: change.before.into(),
            after: change.after.into(),
        }
    }
}

impl From<EspaceAccountDelegation> for DelegationState {
    fn from(state: EspaceAccountDelegation) -> Self {
        Self {
            delegate: state.delegate,
            nonce: state.nonce.into(),
        }
    }
}

impl From<EspaceStandardChange> for StandaloneChange {
    fn from(change: EspaceStandardChange) -> Self {
        match change {
            EspaceStandardChange::Erc20Transfer {
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
            EspaceStandardChange::Erc20Mint {
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
            EspaceStandardChange::Erc20Burn {
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
            EspaceStandardChange::Erc20Approval {
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
            EspaceStandardChange::Erc721Transfer {
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
            EspaceStandardChange::Erc721Mint {
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
            EspaceStandardChange::Erc721Burn {
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
            EspaceStandardChange::Erc721Approval {
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
            EspaceStandardChange::OperatorApproval {
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
            EspaceStandardChange::Erc1155TransferSingle {
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
            EspaceStandardChange::Erc1155MintSingle {
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
            EspaceStandardChange::Erc1155BurnSingle {
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
            EspaceStandardChange::Erc1155TransferBatch {
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
            EspaceStandardChange::Erc1155MintBatch {
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
            EspaceStandardChange::Erc1155BurnBatch {
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
