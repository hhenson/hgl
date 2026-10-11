//! Inert portable native scalar and readonly helper contract data.
use std::collections::BTreeSet;
/// Executable image location projected from a completely validated package contract.
#[derive(Debug, Clone)]
pub struct Image {
    /// Checked provider identity.
    pub provider: String,
    /// Checked semantic interface fingerprint.
    pub fingerprint: String,
    /// Resolved statically compiled target source location.
    pub path: String,
    /// Explicit typed target markers paired with their portable checked contracts.
    pub scalars: Vec<(Scalar, String)>,
}
/// Owning scalar operations advertised by the portable package contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Scalar {
    /// Existing canonical identity, independent of the HGL binding name.
    pub identity: String,
    /// Shared operations available to checked source contexts.
    pub capabilities: BTreeSet<String>,
}
/// An exact ordinary native helper contract, containing no executable code.
#[derive(Debug, Clone)]
pub struct Function {
    /// Readonly parameter names and exact source type expressions.
    pub parameters: Vec<(String, String)>,
    /// Exact owning result type expression.
    pub result: String,
    /// Whether provider exceptions use the ordinary translated error channel.
    pub throws: bool,
    /// Selected target symbol, bound before execution.
    pub symbol: String,
    /// Explicit execution contexts; never required constant evaluation.
    pub phases: BTreeSet<String>,
    /// Observable implementation effects.
    pub effects: BTreeSet<String>,
}
