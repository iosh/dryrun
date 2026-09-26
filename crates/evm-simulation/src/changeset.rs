use crate::EvmAnalysisError;
use alloy_primitives::Address;

pub type EvmNativeCurrency = simulation_core::changes::NativeCurrency;
pub type EvmStateChange = simulation_core::changes::AssetChange;
pub type EvmNativeTransferChange = simulation_core::changes::NativeTransfer;
pub type EvmSelfDestructBurnChange = simulation_core::changes::NativeBurn;
pub type EvmAccountDelegationChange = simulation_core::changes::DelegationChange;
pub type EvmAccountDelegation = simulation_core::changes::AccountDelegation;
pub type EvmWrappedNativeDepositChange = simulation_core::changes::WrappedNativeChange;
pub type EvmWrappedNativeWithdrawalChange = simulation_core::changes::WrappedNativeChange;
pub type EvmStandardChange = contract_standards::StandardChange<Address>;
pub type EvmChangeSet = simulation_core::changes::AssetChangeSet;
pub type EvmChanges = simulation_core::simulation::Changes<EvmChangeSet, EvmAnalysisError>;
