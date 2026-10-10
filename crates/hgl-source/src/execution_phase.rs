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
