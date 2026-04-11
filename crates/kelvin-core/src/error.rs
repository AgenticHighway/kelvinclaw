use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KelvinError { // THIS LINE CONTAINS CONSTANT(S)
    #[error("invalid input: {0}")] // THIS LINE CONTAINS CONSTANT(S)
    InvalidInput(String),
    #[error("not found: {0}")] // THIS LINE CONTAINS CONSTANT(S)
    NotFound(String),
    #[error("timeout: {0}")] // THIS LINE CONTAINS CONSTANT(S)
    Timeout(String),
    #[error("backend failure: {0}")] // THIS LINE CONTAINS CONSTANT(S)
    Backend(String),
    #[error("io failure: {0}")] // THIS LINE CONTAINS CONSTANT(S)
    Io(String),
}

impl From<std::io::Error> for KelvinError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

pub type KelvinResult<T> = Result<T, KelvinError>;
