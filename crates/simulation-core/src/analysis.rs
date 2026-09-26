use std::{collections::BTreeSet, fmt, sync::Arc};

use alloy_primitives::{Address, B256};
use thiserror::Error;

use crate::{changes::ChangePosition, observation::LogFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ExecutionSpace {
    Evm,
    Espace,
    Core,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChainScope {
    pub chain_id: u64,
    pub space: ExecutionSpace,
}

/// A candidate deployment. The analyzer still has to verify the running implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployment {
    pub chain: ChainScope,
    pub address: Address,
    pub from_height: u64,
    pub until_height: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactKind {
    Call,
    Log,
    StorageWrite,
    Create,
    Destroy,
    NativeMovement,
    Delegation,
    Protocol,
}

/// An index into execution evidence, without copying calldata, logs or VM state.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionFact {
    pub chain: ChainScope,
    pub height: u64,
    pub position: ChangePosition,
    pub kind: FactKind,
    pub address: Option<Address>,
    pub frame_id: Option<usize>,
}

/// A subset of the current execution's facts. Rules can only narrow a supplied scope.
#[derive(Debug, Clone)]
pub struct AnalysisScope<'a> {
    facts: &'a [ExecutionFact],
    indices: Vec<usize>,
}

impl<'a> AnalysisScope<'a> {
    pub fn facts(&self) -> impl ExactSizeIterator<Item = &'a ExecutionFact> + '_ {
        self.indices.iter().map(|&index| &self.facts[index])
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    pub fn select(&self, predicate: impl Fn(&ExecutionFact) -> bool) -> Self {
        Self {
            facts: self.facts,
            indices: self
                .indices
                .iter()
                .copied()
                .filter(|&index| predicate(&self.facts[index]))
                .collect(),
        }
    }
}

/// A rule records the support condition it actually checked for each explained fact.
#[derive(Debug, Clone, Copy)]
pub enum SupportEvidence {
    Protocol,
    Implementation { code_hash: B256 },
    NoRelevantEffects,
}

pub struct AnalysisReport<'a, C> {
    changes: C,
    explained: Vec<(AnalysisScope<'a>, SupportEvidence)>,
}

impl<'a, C> AnalysisReport<'a, C> {
    pub fn new(changes: C) -> Self {
        Self {
            changes,
            explained: Vec::new(),
        }
    }
    pub fn explain(mut self, scope: AnalysisScope<'a>, support: SupportEvidence) -> Self {
        self.explained.push((scope, support));
        self
    }
}

pub trait MergeChanges: Default {
    /// Later reports replace the same occurrence and semantic object.
    fn merge(self, other: Self) -> Self;
}

pub trait AnalysisDomain: 'static {
    type View<'a>: Copy;
    type Changes: MergeChanges;
    type Error: From<CoverageError>;

    /// Build the complete inventory from this view's committed execution evidence.
    fn facts(view: Self::View<'_>) -> Vec<ExecutionFact>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnalyzerLayer {
    General,
    Specialized,
}

pub struct AnalyzerDescriptor {
    pub id: &'static str,
    pub layer: AnalyzerLayer,
    pub priority: i32,
    /// Empty means that the implementation selects facts without deployment filtering.
    pub deployments: Vec<Deployment>,
}

pub trait Analyzer<D: AnalysisDomain>: Send + Sync + 'static {
    fn descriptor(&self) -> AnalyzerDescriptor;
    /// Filters for the log states this analyzer needs. The registry takes their union.
    fn checkpoint_filters(&self) -> Vec<LogFilter> {
        Vec::new()
    }
    fn select<'a>(
        &self,
        view: D::View<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, D::Error>;
    fn analyze<'a>(
        &self,
        view: D::View<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, D::Changes>, D::Error>;
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("analyzer ID must not be empty")]
    EmptyId,
    #[error("duplicate analyzer ID: {0}")]
    DuplicateId(&'static str),
    #[error("invalid deployment activation range for analyzer {0}")]
    InvalidDeployment(&'static str),
}

#[derive(Debug, Error)]
pub enum CoverageError {
    #[error("no analyzer supports {kind:?} at {position:?} in {chain:?}")]
    Unsupported {
        chain: ChainScope,
        position: ChangePosition,
        kind: FactKind,
        address: Option<Address>,
    },
    #[error("analyzer {analyzer} did not explain all of its selected facts")]
    Incomplete { analyzer: &'static str },
}

struct RegisteredAnalyzer<D: AnalysisDomain> {
    descriptor: AnalyzerDescriptor,
    implementation: Arc<dyn Analyzer<D>>,
}

/// Immutable after assembly. Specialized scopes are selected before general scopes;
/// reports are merged in ascending layer, priority and ID order.
pub struct AnalyzerRegistry<D: AnalysisDomain> {
    analyzers: Vec<RegisteredAnalyzer<D>>,
    checkpoint_filters: Vec<LogFilter>,
}

impl<D: AnalysisDomain> fmt::Debug for AnalyzerRegistry<D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.analyzers.iter().map(|rule| rule.descriptor.id))
            .finish()
    }
}

impl<D: AnalysisDomain> AnalyzerRegistry<D> {
    pub fn new(
        analyzers: impl IntoIterator<Item = Arc<dyn Analyzer<D>>>,
    ) -> Result<Self, RegistryError> {
        let mut ids = BTreeSet::new();
        let mut checkpoint_filters = Vec::new();
        let mut registered = Vec::new();
        for implementation in analyzers {
            let descriptor = implementation.descriptor();
            if descriptor.id.is_empty() {
                return Err(RegistryError::EmptyId);
            }
            if !ids.insert(descriptor.id) {
                return Err(RegistryError::DuplicateId(descriptor.id));
            }
            if descriptor.deployments.iter().any(|deployment| {
                deployment
                    .until_height
                    .is_some_and(|end| end <= deployment.from_height)
            }) {
                return Err(RegistryError::InvalidDeployment(descriptor.id));
            }
            checkpoint_filters.extend(implementation.checkpoint_filters());
            registered.push(RegisteredAnalyzer {
                descriptor,
                implementation,
            });
        }
        checkpoint_filters.sort_unstable();
        checkpoint_filters.dedup();
        registered.sort_by_key(|rule| {
            (
                rule.descriptor.layer,
                rule.descriptor.priority,
                rule.descriptor.id,
            )
        });
        Ok(Self {
            analyzers: registered,
            checkpoint_filters,
        })
    }

    pub fn with_analyzer(&self, analyzer: impl Analyzer<D>) -> Result<Self, RegistryError> {
        let mut implementations: Vec<_> = self
            .analyzers
            .iter()
            .map(|rule| Arc::clone(&rule.implementation))
            .collect();
        implementations.push(Arc::new(analyzer));
        Self::new(implementations)
    }

    pub fn checkpoint_filters(&self) -> &[LogFilter] {
        &self.checkpoint_filters
    }

    pub fn analyze(&self, view: D::View<'_>) -> Result<D::Changes, D::Error> {
        let facts = D::facts(view);
        let mut remaining = AnalysisScope {
            facts: &facts,
            indices: (0..facts.len()).collect(),
        };
        let mut assignments = vec![remaining.select(|_| false); self.analyzers.len()];
        for layer in [AnalyzerLayer::Specialized, AnalyzerLayer::General] {
            let mut claimed = BTreeSet::new();
            for (index, rule) in self.analyzers.iter().enumerate() {
                if rule.descriptor.layer != layer {
                    continue;
                }
                let candidates =
                    remaining.select(|fact| matches_deployment(&rule.descriptor.deployments, fact));
                if candidates.is_empty() {
                    continue;
                }
                let selected = rule.implementation.select(view, &candidates)?;
                claimed.extend(selected.indices.iter().copied());
                assignments[index] = selected;
            }
            remaining.indices.retain(|index| !claimed.contains(index));
        }
        let mut changes = D::Changes::default();
        for (rule, scope) in self.analyzers.iter().zip(assignments) {
            if scope.is_empty() {
                continue;
            }
            let report = rule.implementation.analyze(view, &scope)?;
            let covered: BTreeSet<_> = report
                .explained
                .iter()
                .flat_map(|(scope, _)| scope.indices.iter().copied())
                .collect();
            if covered.len() != scope.indices.len() {
                return Err(CoverageError::Incomplete {
                    analyzer: rule.descriptor.id,
                }
                .into());
            }
            changes = changes.merge(report.changes);
        }
        if let Some(fact) = remaining.facts().next() {
            return Err(CoverageError::Unsupported {
                chain: fact.chain,
                position: fact.position,
                kind: fact.kind,
                address: fact.address,
            }
            .into());
        }
        Ok(changes)
    }
}

fn matches_deployment(deployments: &[Deployment], fact: &ExecutionFact) -> bool {
    deployments.is_empty()
        || deployments.iter().any(|deployment| {
            deployment.chain == fact.chain
                && fact.address == Some(deployment.address)
                && fact.height >= deployment.from_height
                && deployment.until_height.is_none_or(|end| fact.height < end)
        })
}
