//! Ordered checked test setup and independently owned graph inputs.
use hgl_rust_ir::{Statement, Value};
use hgl_source::Ty;

/// One lexical test scope, independent of fresh graph lifetimes.
#[derive(Debug)]
pub struct Test {
    /// Qualified source name.
    pub name: String,
    /// Checked source-order operations.
    pub steps: Vec<Step>,
}
/// Setup, assertion or fresh graph execution.
#[derive(Debug)]
pub enum Step {
    /// Checked ordinary source statement.
    Ordinary(Statement),
    /// Checked ordinary boolean assertion.
    Assert(Value),
    /// One independently prepared graph run.
    Eval(Evaluation),
    /// Retain a graph result as an immutable, contextual nullable sequence.
    BindEval(usize, Ty, Evaluation),
    /// Checked condition and lexical alternatives, evaluated once.
    If(Value, Vec<Self>, Vec<Self>),
}
/// One selected graph and its source-order inputs and expectations.
#[derive(Debug)]
pub struct Evaluation {
    /// Compile-time index of its generated graph plan.
    pub case: usize,
    /// Supplied arguments in written order.
    pub arguments: Vec<Argument>,
    /// Checked dense expectations, evaluated after graph completion.
    pub expected: Option<Vec<Option<Value>>>,
}
/// One value-retaining argument preparation boundary.
#[derive(Debug)]
pub enum Argument {
    /// Ordinary exact typed node configuration.
    Constant {
        /// Prepared configuration position.
        binding: usize,
        /// Checked source expression.
        value: Value,
    },
    /// Dense temporal publications lowered to ordinary timed entries.
    Dense {
        /// Declared parameter name for pre-start diagnostics.
        parameter: String,
        /// Prepared configuration position.
        binding: usize,
        /// Exact originating temporal shape.
        shape: Ty,
        /// Checked declaration-owned `TimedValue` specialization.
        entry_type: Ty,
        /// Written cells; absence is not a value.
        slots: Vec<Option<Value>>,
    },
}
/// Constructed owning configurations for a single fresh graph.
#[derive(Debug)]
pub struct PreparedEval {
    /// Independent values in prepared binding order.
    pub arguments: Vec<Value>,
    /// Dense input horizon independent of expectations.
    pub input_length: usize,
}

/// Owning sparse captures with a dense logical horizon.
#[derive(Debug)]
pub struct CapturedEval {
    /// Input/output horizon independent of expectations.
    pub length: usize,
    /// Present publications in strictly increasing cycle order.
    pub ticks: Vec<(usize, Value)>,
}
