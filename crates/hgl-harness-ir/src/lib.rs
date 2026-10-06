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
    /// Require one exact structured execution error after successful teardown.
    Raises(String, Vec<Self>),
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

/// A graph error remains distinguishable from an assertion or build failure.
#[derive(Debug)]
pub enum Failure {
    /// An error propagated by graph execution, including its cleanup context.
    Execution(Box<hgl_node_error::NodeError>),
    /// An unclassified failure, including every test assertion failure.
    Other(String),
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Other(message.into())
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Execution(error) => write!(f, "{error:?}"),
            Self::Other(message) => f.write_str(message),
        }
    }
}
impl Failure {
    /// Cleanup-only errors and errors followed by failed cleanup cannot match.
    pub fn matches(&self, code: &str) -> bool {
        matches!(self, Self::Execution(error) if error.code == Some(code)
            && error.phase != hgl_node_error::Phase::Stop && error.cleanup.is_empty())
    }
}

/// Checked tests with their independently selected graph plans.
#[derive(Debug)]
pub struct Suite {
    /// Named lexical tests, in source order.
    pub tests: Vec<Test>,
    /// Generated graph plans addressed by each evaluation's case index.
    pub plans: Vec<hgl_rust_ir::Plan>,
}
impl Suite {
    /// Retain requested short or qualified names, rejecting every unknown selector.
    pub fn select(&mut self, names: &[&str]) -> Result<(), String> {
        let matches = |test: &Test, name: &str| {
            test.name == name || test.name.rsplit("::").next() == Some(name)
        };
        for name in names {
            if !self.tests.iter().any(|test| matches(test, name)) {
                return Err(format!("unknown test selector {name}"));
            }
        }
        if !names.is_empty() {
            self.tests
                .retain(|test| names.iter().any(|name| matches(test, name)));
        }
        Ok(())
    }
}
