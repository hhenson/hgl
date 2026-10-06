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
}

impl NodeError {
    /// A failure with only its message. A node does not know where it sits:
    /// the graph fills in `node`, `label` and `phase` as the failure leaves
    /// the hook.
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "a NodeResult carries its error boxed, so a node writes `Err(NodeError::new(..))`"
    )]
    pub fn new(message: impl Into<String>) -> Box<Self> {
        Box::new(Self {
            node: NodeId(0),
            label: String::new(),
            phase: Phase::Eval,
            message: message.into(),
            code: None,
            cleanup: Vec::new(),
        })
    }
}

impl NodeError {
    /// Attach a specified semantic identifier without changing graph context.
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "NodeResult owns a boxed error; coded preserves the same error channel as new"
    )]
    pub fn coded(message: impl Into<String>, code: &'static str) -> Box<Self> {
        let mut error = Self::new(message);
        error.code = Some(code);
        error
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
