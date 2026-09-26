use std::{collections::BTreeSet, marker::PhantomData, sync::Arc};

use alloy_primitives::{Address, address, keccak256};
use contract_standards::analysis::{
    self, AnalysisError, VerifiedChange,
    view::{ContractView, FrameAction, TokenView},
};

use crate::{analysis::*, changes::ChangePosition, observation::LogFilter};

/// Backend adapters lend their existing state readers and preserve their address/Space types.
pub trait ContractAnalysisDomain: AnalysisDomain {
    fn token_view<'a>(
        view: Self::View<'a>,
        chain: ChainScope,
    ) -> Result<Box<dyn TokenView + 'a>, Self::Error>;
    fn insert_contract_change(
        view: Self::View<'_>,
        changes: &mut Self::Changes,
        chain: ChainScope,
        change: VerifiedChange,
    ) -> Result<(), Self::Error>;
    fn contract_error(error: AnalysisError) -> Self::Error;
}

pub use contract_standards::analysis::ReviewedStandardImplementation;

/// Adapts one reviewed implementation to the shared standard algorithms.
/// Matching contract facts take precedence over the general contract fallback.
pub struct StandardAnalyzer<D> {
    id: &'static str,
    layer: AnalyzerLayer,
    implementation: Box<dyn ReviewedStandardImplementation>,
    domain: PhantomData<fn() -> D>,
}

impl<D> StandardAnalyzer<D> {
    pub fn new(id: &'static str, implementation: impl ReviewedStandardImplementation) -> Self {
        Self {
            id,
            layer: AnalyzerLayer::Specialized,
            implementation: Box::new(implementation),
            domain: PhantomData,
        }
    }
}

impl<D: ContractAnalysisDomain> Analyzer<D> for StandardAnalyzer<D> {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: self.id,
            layer: self.layer,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn checkpoint_filters(&self) -> Vec<LogFilter> {
        standard_checkpoint_filters()
    }
    fn select<'a>(
        &self,
        view: D::View<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, D::Error> {
        let candidates = candidates.select(|fact| is_contract_fact(fact.kind));
        if self.layer == AnalyzerLayer::General {
            return Ok(candidates);
        }
        let mut matched = BTreeSet::new();
        for (chain, contract) in contracts(&candidates) {
            let adapter = D::token_view(view, chain)?;
            let facts =
                candidates.select(|fact| fact.chain == chain && fact.address == Some(contract));
            let positions = execution_positions(&facts);
            let contract_view = ContractView {
                execution: adapter.as_ref(),
                address: contract,
                positions: &positions,
            };
            if self
                .implementation
                .matches(&contract_view, contract)
                .map_err(D::contract_error)?
            {
                matched.insert((chain, contract));
            }
        }
        Ok(candidates.select(|fact| {
            fact.address
                .is_some_and(|address| matched.contains(&(fact.chain, address)))
        }))
    }
    fn analyze<'a>(
        &self,
        view: D::View<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, D::Changes>, D::Error> {
        let mut changes = D::Changes::default();
        let mut coverage = Vec::new();
        for (chain, contract) in contracts(scope) {
            let adapter = D::token_view(view, chain)?;
            let facts = scope.select(|fact| fact.chain == chain && fact.address == Some(contract));
            let positions = execution_positions(&facts);
            let contract_view = ContractView {
                execution: adapter.as_ref(),
                address: contract,
                positions: &positions,
            };
            let matched = self.layer == AnalyzerLayer::Specialized
                || self
                    .implementation
                    .matches(&contract_view, contract)
                    .map_err(D::contract_error)?;
            let support = if matched {
                self.implementation
                    .verify_support(&contract_view, contract)
                    .map_err(D::contract_error)?;
                for change in analysis::analyze(&contract_view, None).map_err(D::contract_error)? {
                    D::insert_contract_change(view, &mut changes, chain, change)?;
                }
                SupportEvidence::Implementation {
                    code_hash: self.implementation.code_hash(),
                }
            } else {
                verify_no_effects(&contract_view, contract, &facts).map_err(D::contract_error)?;
                SupportEvidence::NoRelevantEffects
            };
            coverage.push((facts, support));
        }
        let mut report = AnalysisReport::new(changes);
        for (facts, support) in coverage {
            report = report.explain(facts, support);
        }
        Ok(report)
    }
}

pub struct Weth9Analyzer<D> {
    id: &'static str,
    deployments: Vec<Deployment>,
    domain: PhantomData<fn() -> D>,
}

impl<D> Weth9Analyzer<D> {
    pub fn new(id: &'static str, deployments: Vec<Deployment>) -> Self {
        Self {
            id,
            deployments,
            domain: PhantomData,
        }
    }
    pub fn ethereum_mainnet() -> Self {
        Self::new(
            "canonical-weth9",
            vec![Deployment {
                chain: ChainScope {
                    chain_id: 1,
                    space: ExecutionSpace::Evm,
                },
                address: address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                from_height: 4_719_568,
                until_height: None,
            }],
        )
    }
}

impl<D: ContractAnalysisDomain> Analyzer<D> for Weth9Analyzer<D> {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: self.id,
            layer: AnalyzerLayer::Specialized,
            priority: 0,
            deployments: self.deployments.clone(),
        }
    }
    fn checkpoint_filters(&self) -> Vec<LogFilter> {
        let mut filters = standard_checkpoint_filters();
        for deployment in &self.deployments {
            for signature in ["Deposit(address,uint256)", "Withdrawal(address,uint256)"] {
                filters.push(LogFilter {
                    address: Some(deployment.address),
                    topic0: keccak256(signature),
                });
            }
        }
        filters
    }
    fn select<'a>(
        &self,
        _: D::View<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, D::Error> {
        Ok(candidates.select(|fact| is_contract_fact(fact.kind)))
    }
    fn analyze<'a>(
        &self,
        view: D::View<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, D::Changes>, D::Error> {
        let mut changes = D::Changes::default();
        if scope
            .facts()
            .any(|fact| matches!(fact.kind, FactKind::Create | FactKind::Destroy))
        {
            return Err(D::contract_error(AnalysisError::unsupported(
                "WETH9 creation or destruction is outside the deployed-runtime rule",
            )));
        }
        for (chain, contract) in contracts(scope) {
            let adapter = D::token_view(view, chain)?;
            let facts = scope.select(|fact| fact.chain == chain && fact.address == Some(contract));
            let positions = execution_positions(&facts);
            let contract_view = ContractView {
                execution: adapter.as_ref(),
                address: contract,
                positions: &positions,
            };
            for change in
                analysis::weth9::analyze(&contract_view, contract).map_err(D::contract_error)?
            {
                D::insert_contract_change(view, &mut changes, chain, change)?;
            }
        }
        Ok(AnalysisReport::new(changes).explain(
            scope.clone(),
            SupportEvidence::Implementation {
                code_hash: analysis::weth9::CODE_HASH,
            },
        ))
    }
}

pub fn default_contract_analyzers<D: ContractAnalysisDomain>() -> Vec<Arc<dyn Analyzer<D>>> {
    vec![
        Arc::new(StandardAnalyzer::<D> {
            id: "contract-standards",
            layer: AnalyzerLayer::General,
            implementation: Box::new(analysis::dai::Dai),
            domain: PhantomData,
        }),
        Arc::new(Weth9Analyzer::<D>::ethereum_mainnet()),
    ]
}

fn standard_checkpoint_filters() -> Vec<LogFilter> {
    contract_standards::supported_event_topics()
        .iter()
        .map(|&topic0| LogFilter {
            address: None,
            topic0,
        })
        .collect()
}

fn is_contract_fact(kind: FactKind) -> bool {
    matches!(
        kind,
        FactKind::Call
            | FactKind::Log
            | FactKind::StorageWrite
            | FactKind::Create
            | FactKind::Destroy
    )
}

fn contracts(scope: &AnalysisScope<'_>) -> BTreeSet<(ChainScope, Address)> {
    scope
        .facts()
        .filter_map(|fact| fact.address.map(|address| (fact.chain, address)))
        .collect()
}

fn execution_positions(facts: &AnalysisScope<'_>) -> BTreeSet<usize> {
    facts
        .facts()
        .filter_map(|fact| match fact.position {
            ChangePosition::Execution(position) => Some(position),
            ChangePosition::BeforeExecution | ChangePosition::Settlement => None,
        })
        .collect()
}

fn verify_no_effects(
    view: &dyn TokenView,
    contract: Address,
    facts: &AnalysisScope<'_>,
) -> Result<(), AnalysisError> {
    if facts.facts().any(|fact| fact.kind != FactKind::Call)
        || view.committed_logs().next().is_some()
        || view.storage_writes().next().is_some()
    {
        return Err(AnalysisError::unsupported(format!(
            "contract {contract} has no reviewed implementation for its committed effects"
        )));
    }
    // The absence of writes does not establish the semantics of unknown code.
    // A code-free call is the only contract scope this fallback can explain.
    let mut addresses = BTreeSet::from([contract]);
    for frame in view
        .committed_frames()
        .filter(|frame| view.is_in_scope(frame.position))
    {
        let FrameAction::Call {
            target,
            bytecode_address,
            ..
        } = frame.action
        else {
            continue;
        };
        if target != contract {
            continue;
        }
        if frame.code_hash.is_some_and(|hash| hash != keccak256([])) {
            return Err(AnalysisError::unsupported(format!(
                "no reviewed implementation for contract {contract}"
            )));
        }
        addresses.insert(bytecode_address);
    }
    for address in addresses {
        for state in [view.initial(), view.finalized()] {
            if !state.code(address)?.is_empty() {
                return Err(AnalysisError::unsupported(format!(
                    "no reviewed implementation for contract {address}"
                )));
            }
        }
    }
    Ok(())
}
