use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("contract state read failed: {source}")]
    State {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("unsupported contract behavior: {details}")]
    Unsupported { details: String },
    #[error("incomplete contract evidence: {details}")]
    IncompleteEvidence { details: String },
    #[error("contract validation failed at {position:?}: {details}")]
    Validation {
        position: Option<usize>,
        details: String,
    },
}

impl AnalysisError {
    pub fn state(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::State {
            source: Box::new(source),
        }
    }
    pub fn unsupported(details: impl Into<String>) -> Self {
        Self::Unsupported {
            details: details.into(),
        }
    }
}

pub(super) fn validation_error(details: impl Into<String>) -> AnalysisError {
    AnalysisError::Validation {
        position: None,
        details: details.into(),
    }
}
pub(super) fn validation_error_at(position: usize, details: impl Into<String>) -> AnalysisError {
    AnalysisError::Validation {
        position: Some(position),
        details: details.into(),
    }
}
