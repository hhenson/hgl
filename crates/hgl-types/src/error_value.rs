//! Canonical diagnostic data retains provenance independently of semantic equality.
use crate::EngineTime;
/// The original graph failure location, retained by the diagnostic value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    /// Run-local node/graph instance identity.
    pub node: String,
    /// evaluate, start or stop.
    pub phase: String,
    /// Logical time at the original failing hook.
    pub evaluation_time: EngineTime,
}
/// An ordinary diagnostic is data until explicitly propagated into graph failure.
#[derive(Debug, Clone)]
pub struct ErrorValue {
    /// The original user-facing diagnostic.
    pub message: String,
    /// An absent code has no replacement alias.
    pub code: Option<String>,
    /// Locally constructed diagnostics have no provenance.
    pub context: Option<ErrorContext>,
}
impl PartialEq for ErrorValue {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message && self.code == other.code
    }
}
impl Eq for ErrorValue {}
