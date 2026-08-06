#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome<Details, Failure> {
    Success(Details),
    Failed { details: Details, failure: Failure },
    NotExecuted(Failure),
}
