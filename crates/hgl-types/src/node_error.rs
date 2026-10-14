//! Graph error context and stable semantic identities.
/// A node's position in its graph's rank order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

/// Shared translated errors for hooks and typed capability operations. The
/// default unit hook result occupies one word because its error is boxed.
pub type NodeResult<T = ()> = Result<T, Box<NodeError>>;

/// Which hook was running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The start hook.
    Start,
    /// The evaluation hook.
    Eval,
    /// The stop hook.
    Stop,
}

/// A failure that left a node: which node, in which hook, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeError {
    /// The failing node's rank.
    pub node: NodeId,
    /// The failing node's label.
    pub label: String,
    /// The hook that failed.
    pub phase: Phase,
    /// Why, in the node's words.
    pub message: String,
    /// Stable language execution identifier, assigned only at its origin.
    pub code: Option<&'static str>,
    /// Failures from ordinary teardown after this primary error.
    pub cleanup: Vec<Self>,
    /// Canonical language diagnostic, separate from private operational ABI flags.
    pub diagnostic: Option<crate::ErrorValue>,
}

impl NodeError {
    /// A failure with only its message. A node does not know where it sits:
    /// the graph fills in `node`, `label` and `phase` as the failure leaves
    /// the hook.
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            node: NodeId(0),
            label: String::new(),
            phase: Phase::Eval,
            message: message.into(),
            code: None,
            cleanup: Vec::new(),
            diagnostic: None,
        })
    }
    /// Attach a specified semantic identifier without changing graph context.
    pub fn coded(message: impl Into<String>, code: &'static str) -> Box<Self> {
        let mut error = Self::new(message);
        error.code = Some(code);
        error
    }
    /// Enter the execution channel without inventing a code for an uncoded diagnostic.
    pub fn from_error(value: crate::ErrorValue) -> Self {
        Self {
            node: NodeId(0),
            label: String::new(),
            phase: Phase::Eval,
            message: value.message.clone(),
            code: None,
            cleanup: Vec::new(),
            diagnostic: Some(value),
        }
    }
    /// Resolve the original execution identifier, including result-propagated codes.
    pub fn execution_code(&self) -> Option<&str> {
        self.diagnostic
            .as_ref()
            .and_then(|value| value.code.as_deref())
            .or(self.code)
    }
    /// Retain the canonical diagnostic as owned failure data.
    pub fn error_value(&self) -> crate::ErrorValue {
        self.diagnostic
            .clone()
            .unwrap_or_else(|| crate::ErrorValue {
                message: self.message.clone(),
                code: self.code.map(str::to_owned),
                context: None,
            })
    }
}

/// Preserve the primary failure and every reported cleanup failure.
pub fn finish(primary: NodeResult, cleanup: NodeResult) -> NodeResult {
    match (primary, cleanup) {
        (Err(mut primary), Err(cleanup)) => {
            primary.cleanup.push(*cleanup);
            Err(primary)
        }
        (Ok(()), result) | (result, Ok(())) => result,
    }
}
