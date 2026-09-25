use std::future::Future;

use tokio::runtime::Handle;

use crate::transaction::TransactionInput;

/// Availability of the complete set of verified transaction changes.
#[derive(Debug)]
pub enum Changes<T, E> {
    Complete(T),
    NotAnalyzed,
    Unavailable(E),
}

impl<T, E> From<Result<T, E>> for Changes<T, E> {
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(changes) => Self::Complete(changes),
            Err(error) => Self::Unavailable(error),
        }
    }
}

/// A rejection retains exactly the input and context available when it was decided.
#[derive(Debug)]
pub struct RejectedSimulation<C, T, P, R> {
    context: Option<C>,
    transaction: TransactionInput<T, P>,
    rejection: R,
}

impl<C, T, P, R> RejectedSimulation<C, T, P, R> {
    pub fn context(&self) -> Option<&C> {
        self.context.as_ref()
    }

    pub fn transaction(&self) -> &TransactionInput<T, P> {
        &self.transaction
    }

    pub fn rejection(&self) -> &R {
        &self.rejection
    }

    pub fn into_parts(self) -> (Option<C>, TransactionInput<T, P>, R) {
        (self.context, self.transaction, self.rejection)
    }
}

#[derive(Debug)]
pub struct ExecutedSimulation<C, T, O, S, E> {
    context: C,
    transaction: T,
    outcome: O,
    changes: Changes<S, E>,
}

impl<C, T, O, S, E> ExecutedSimulation<C, T, O, S, E> {
    pub fn context(&self) -> &C {
        &self.context
    }

    pub fn transaction(&self) -> &T {
        &self.transaction
    }

    pub fn outcome(&self) -> &O {
        &self.outcome
    }

    pub fn changes(&self) -> &Changes<S, E> {
        &self.changes
    }

    pub fn into_parts(self) -> (C, T, O, Changes<S, E>) {
        (self.context, self.transaction, self.outcome, self.changes)
    }
}

/// The two reliable exits of a simulation. System failures use the outer Result.
#[derive(Debug)]
pub enum Simulation<C, T, P, O, R, S, E> {
    Rejected(RejectedSimulation<C, T, P, R>),
    Executed(ExecutedSimulation<C, T, O, S, E>),
}

impl<C, T, P, O, R, S, E> Simulation<C, T, P, O, R, S, E> {
    /// Transforms complete change items without changing execution or availability.
    pub fn map_changes<U>(self, map: impl FnOnce(S) -> U) -> Simulation<C, T, P, O, R, U, E> {
        match self.try_map_changes(|changes| Ok::<_, std::convert::Infallible>(map(changes))) {
            Ok(result) => result,
            Err(never) => match never {},
        }
    }

    pub fn try_map_changes<U, F>(
        self,
        map: impl FnOnce(S) -> Result<U, F>,
    ) -> Result<Simulation<C, T, P, O, R, U, E>, F> {
        Ok(match self {
            Self::Rejected(rejected) => Simulation::Rejected(rejected),
            Self::Executed(executed) => {
                let (context, transaction, outcome, changes) = executed.into_parts();
                let changes = match changes {
                    Changes::Complete(changes) => Changes::Complete(map(changes)?),
                    Changes::NotAnalyzed => Changes::NotAnalyzed,
                    Changes::Unavailable(error) => Changes::Unavailable(error),
                };
                Simulation::Executed(ExecutedSimulation {
                    context,
                    transaction,
                    outcome,
                    changes,
                })
            }
        })
    }
}

/// A backend's preparation exit, before any local user transaction is executed.
pub enum Preparation<C, T, P, R, D> {
    Ready {
        context: C,
        transaction: T,
        execution: D,
    },
    Rejected {
        context: Option<C>,
        transaction: TransactionInput<T, P>,
        rejection: R,
    },
}

/// Execution integrations return facts from the same transaction, or its rejection.
pub enum Execution<D, R> {
    Executed(D),
    Rejected(R),
}

/// A read-only view tied to one prepared transaction and its execution evidence.
pub struct AnalysisView<'a, C, T, D> {
    context: &'a C,
    transaction: &'a T,
    execution: &'a D,
}

impl<'a, C, T, D> AnalysisView<'a, C, T, D> {
    pub fn context(&self) -> &'a C {
        self.context
    }

    pub fn transaction(&self) -> &'a T {
        self.transaction
    }

    pub fn execution(&self) -> &'a D {
        self.execution
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("simulation requires an active Tokio runtime")]
    Unavailable,
    #[error("blocking simulation task terminated unexpectedly: {0}")]
    Task(#[source] tokio::task::JoinError),
}

/// Static chain integration. VM state never leaves the blocking execution task.
pub trait SimulationBackend: Send + Sync + 'static {
    type Request: Send;
    type Context: Send + 'static;
    type Transaction: Send + 'static;
    type TransactionRequest: Send + 'static;
    type Rejection: Send + 'static;
    type Prepared: Send + 'static;
    type Evidence;
    type Outcome: Send + 'static;
    type ChangeSet: Send + 'static;
    type AnalysisError: Send + 'static;
    type Error: From<RuntimeError> + Send + 'static;

    fn prepare(
        &self,
        request: Self::Request,
    ) -> impl Future<Output = Result<PreparationFor<Self>, Self::Error>> + Send;

    fn execute(
        &self,
        transaction: &Self::Transaction,
        prepared: Self::Prepared,
        runtime: Handle,
    ) -> Result<Execution<Self::Evidence, Self::Rejection>, Self::Error>;

    fn is_success(&self, evidence: &Self::Evidence) -> bool;

    fn analyze(
        &self,
        view: AnalysisView<'_, Self::Context, Self::Transaction, Self::Evidence>,
    ) -> Result<Self::ChangeSet, Self::AnalysisError>;

    fn into_outcome(&self, evidence: Self::Evidence) -> Self::Outcome;
}

pub type PreparationFor<B> = Preparation<
    <B as SimulationBackend>::Context,
    <B as SimulationBackend>::Transaction,
    <B as SimulationBackend>::TransactionRequest,
    <B as SimulationBackend>::Rejection,
    <B as SimulationBackend>::Prepared,
>;

pub type SimulationFor<B> = Simulation<
    <B as SimulationBackend>::Context,
    <B as SimulationBackend>::Transaction,
    <B as SimulationBackend>::TransactionRequest,
    <B as SimulationBackend>::Outcome,
    <B as SimulationBackend>::Rejection,
    <B as SimulationBackend>::ChangeSet,
    <B as SimulationBackend>::AnalysisError,
>;

/// Prepares once, executes the user transaction once, and analyzes only success.
pub async fn simulate<B: SimulationBackend>(
    backend: B,
    request: B::Request,
) -> Result<SimulationFor<B>, B::Error> {
    let runtime = Handle::try_current().map_err(|_| RuntimeError::Unavailable)?;
    let (context, transaction, prepared) = match backend.prepare(request).await? {
        Preparation::Rejected {
            context,
            transaction,
            rejection,
        } => {
            return Ok(Simulation::Rejected(RejectedSimulation {
                context,
                transaction,
                rejection,
            }));
        }
        Preparation::Ready {
            context,
            transaction,
            execution,
        } => (context, transaction, execution),
    };
    let blocking_runtime = runtime.clone();
    runtime
        .spawn_blocking(move || {
            let evidence = match backend.execute(&transaction, prepared, blocking_runtime)? {
                Execution::Rejected(rejection) => {
                    return Ok(Simulation::Rejected(RejectedSimulation {
                        context: Some(context),
                        transaction: TransactionInput::Complete(transaction),
                        rejection,
                    }));
                }
                Execution::Executed(evidence) => evidence,
            };
            let changes = if backend.is_success(&evidence) {
                backend
                    .analyze(AnalysisView {
                        context: &context,
                        transaction: &transaction,
                        execution: &evidence,
                    })
                    .into()
            } else {
                Changes::NotAnalyzed
            };
            Ok(Simulation::Executed(ExecutedSimulation {
                context,
                transaction,
                outcome: backend.into_outcome(evidence),
                changes,
            }))
        })
        .await
        .map_err(RuntimeError::Task)?
}
