#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<Details, Failure> {
    Success(Details),
    Failed { details: Details, failure: Failure },
    NotExecuted(Failure),
}
