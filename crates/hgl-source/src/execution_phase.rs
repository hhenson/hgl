//! Cold lifecycle phase metadata shared by checked native value binding.
/// Value execution phases of one checked node lifecycle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Node startup value expressions.
    Start,
    #[default]
    /// Node evaluation value expressions.
    Evaluation,
    /// Node shutdown value expressions.
    Stop,
}
impl ExecutionPhase {
    /// Exact checked provider phase spelling, used only during cold binding.
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Evaluation => "evaluation",
            Self::Stop => "stop",
        }
    }
}

/// Typed operations supplied only by an executed host, never source checking.
pub trait ExecutionContext<V, N, E> {
    /// Materialize one temporal recipe in written execution order.
    fn temporal(&mut self, recipe: &crate::TemporalLiteral) -> Result<crate::Literal, E>;
    /// Execute a checked native binding; absent hosts leave the call deferred.
    fn native(&mut self, _: &N, _: &[V]) -> Option<Result<V, E>> {
        None
    }
}
impl<V, N, E, F: FnMut(&crate::TemporalLiteral) -> Result<crate::Literal, E>>
    ExecutionContext<V, N, E> for F
{
    fn temporal(&mut self, recipe: &crate::TemporalLiteral) -> Result<crate::Literal, E> {
        self(recipe)
    }
}
