//! Ordinary scalar operations and their phase-independent failure contract.
use std::fmt;

/// Failure during ordinary evaluation, separated from an unsupported operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// An admitted operation failed when evaluated.
    Operation(String),
    /// A provider-dependent value needs the host construction context.
    ContextRequired,
    /// The checked IR cannot be executed by this ordinary evaluator.
    Unsupported(String),
}
impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(message) | Self::Unsupported(message) => f.write_str(message),
            Self::ContextRequired => {
                f.write_str("temporal literal requires a construction context")
            }
        }
    }
}
impl std::error::Error for EvalError {}
impl From<String> for EvalError {
    fn from(message: String) -> Self {
        Self::Operation(message)
    }
}
fn unsupported(message: &str) -> EvalError {
    EvalError::Unsupported(message.into())
}
mod scalar;
pub use scalar::{binary, unary};
