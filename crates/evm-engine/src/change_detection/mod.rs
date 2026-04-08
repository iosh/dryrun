mod detectors;
mod log_parsing;

use std::collections::HashMap;

use alloy_primitives::Address;

use crate::{
    Change, EvmExecutionStatus, change_observation::Observation, execution::ExecutionArtifacts,
};

use self::detectors::{
    ApprovalDetector, ApprovalForAllDetector, Erc1155TransferDetector, NativeTransferDetector,
    StandardTransferDetector,
};
#[cfg(test)]
use self::log_parsing::{
    approval_for_all_topic0, approval_topic0, transfer_batch_topic0, transfer_single_topic0,
    transfer_topic0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractKind {
    Erc721,
    Erc1155,
    FungibleLike,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Erc20Metadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

// Detectors ask for external facts through this trait so the semantic pipeline
// can stay independent from the concrete execution backend.
pub(crate) trait DetectionSupport {
    fn resolve_contract_kind(&mut self, contract_address: Address) -> ContractKind;

    fn load_erc20_metadata(&mut self, token_address: Address) -> Erc20Metadata;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DetectionOutcome {
    NotHandled,
    Handled(Vec<Change>),
}

impl DetectionOutcome {
    pub(super) fn handled(change: Change) -> Self {
        Self::Handled(vec![change])
    }

    pub(super) fn ignored() -> Self {
        Self::Handled(Vec::new())
    }
}

// A detector either declines an observation or consumes it. Consuming with an
// empty vector means the observation was recognized but intentionally filtered
// out from the final change list.
pub(crate) trait ObservationDetector: Send + Sync {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome;
}

pub(crate) struct ChangeDetectionPipeline {
    contract_detectors: Vec<ContractDetector>,
    standard_detectors: Vec<Box<dyn ObservationDetector>>,
}

impl ChangeDetectionPipeline {
    // Contract-scoped overrides run before the built-in standard detectors. The
    // first detector that consumes an observation wins.
    pub(crate) fn builtin() -> Self {
        Self {
            contract_detectors: Vec::new(),
            standard_detectors: vec![
                Box::new(NativeTransferDetector),
                Box::new(StandardTransferDetector),
                Box::new(Erc1155TransferDetector),
                Box::new(ApprovalDetector),
                Box::new(ApprovalForAllDetector),
            ],
        }
    }

    #[cfg(test)]
    pub(crate) fn register_contract_detector(
        &mut self,
        chain_id: Option<u64>,
        contract_address: Address,
        detector: Box<dyn ObservationDetector>,
    ) {
        self.contract_detectors.push(ContractDetector {
            chain_id,
            contract_address,
            detector,
        });
    }

    pub(crate) fn extract_changes(
        &self,
        artifacts: &ExecutionArtifacts,
        support: &mut dyn DetectionSupport,
    ) -> Vec<Change> {
        if !matches!(artifacts.status, EvmExecutionStatus::Success) {
            return Vec::new();
        }

        let mut context = DetectionContext::new(artifacts.chain_id, support);
        let mut changes = Vec::new();

        for observation in &artifacts.observations {
            if let DetectionOutcome::Handled(detected_changes) =
                self.detect_observation(observation, &mut context)
            {
                changes.extend(detected_changes);
            }
        }

        changes
    }

    fn detect_observation(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        for detector in &self.contract_detectors {
            let outcome = detector.detect(observation, context);

            if matches!(&outcome, DetectionOutcome::NotHandled) {
                continue;
            }

            return outcome;
        }

        for detector in &self.standard_detectors {
            let outcome = detector.detect(observation, context);

            if matches!(&outcome, DetectionOutcome::NotHandled) {
                continue;
            }

            return outcome;
        }

        DetectionOutcome::NotHandled
    }
}

// Caches best-effort contract classification and token metadata for a single
// extraction run so repeated observations do not re-probe the same address.
pub(crate) struct DetectionContext<'a> {
    chain_id: u64,
    contract_kinds: HashMap<Address, ContractKind>,
    erc20_metadata_by_address: HashMap<Address, Erc20Metadata>,
    support: &'a mut dyn DetectionSupport,
}

impl<'a> DetectionContext<'a> {
    fn new(chain_id: u64, support: &'a mut dyn DetectionSupport) -> Self {
        Self {
            chain_id,
            contract_kinds: HashMap::new(),
            erc20_metadata_by_address: HashMap::new(),
            support,
        }
    }

    pub(super) fn contract_kind(&mut self, contract_address: Address) -> ContractKind {
        if let Some(kind) = self.contract_kinds.get(&contract_address) {
            return *kind;
        }

        let kind = self.support.resolve_contract_kind(contract_address);
        self.contract_kinds.insert(contract_address, kind);
        kind
    }

    pub(super) fn erc20_metadata(&mut self, token_address: Address) -> Erc20Metadata {
        if let Some(metadata) = self.erc20_metadata_by_address.get(&token_address) {
            return metadata.clone();
        }

        let metadata = self.support.load_erc20_metadata(token_address);
        self.erc20_metadata_by_address
            .insert(token_address, metadata.clone());
        metadata
    }
}

struct ContractDetector {
    chain_id: Option<u64>,
    contract_address: Address,
    detector: Box<dyn ObservationDetector>,
}

impl ContractDetector {
    fn detect(
        &self,
        observation: &Observation,
        context: &mut DetectionContext<'_>,
    ) -> DetectionOutcome {
        let Observation::Log { address, .. } = observation else {
            return DetectionOutcome::NotHandled;
        };

        if self
            .chain_id
            .is_some_and(|chain_id| chain_id != context.chain_id)
        {
            return DetectionOutcome::NotHandled;
        }

        if *address != self.contract_address {
            return DetectionOutcome::NotHandled;
        }

        self.detector.detect(observation, context)
    }
}

#[cfg(test)]
mod tests;
