//! Checked compiler/backend data boundary; source validation belongs to the frontend.
use hgl_source::{Literal, Ty};

/// A typed expression after frontend resolution.
#[derive(Debug, Clone)]
pub struct Value {
    /// Resolved payload or endpoint type.
    pub ty: Ty,
    /// Checked expression form.
    pub kind: Kind,
}
impl Value {
    /// Pair a checked expression with its resolved type.
    pub fn new(ty: Ty, kind: Kind) -> Self {
        Self { ty, kind }
    }
}
/// Checked expressions and frontend-only binding markers.
#[derive(Debug, Clone)]
pub enum Kind {
    /// Ordered sparse constructor parts with exact originating type in Value.ty.
    Delta(Vec<DeltaEntry>),
    /// Evaluation-local readonly publication observation, without ownership.
    ObservedLocal(usize),
    /// Typed unavailable result after a recorded wiring operation failure.
    WiringFailure(String),
    /// Independently owned ordinary list construction.
    List(Vec<Value>),
    /// Ordinary indexed projection preserving its parent's authority.
    Index(Box<Value>, Box<Value>),
    /// Ordinary list length observation.
    Length(Box<Value>),
    /// Retain and append to writable unbounded list access.
    Push(Box<Value>, Box<Value>),
    /// Direct ordinary invocation, with readonly arguments and lexical body.
    ValueCall(Vec<Value>, Vec<Statement>),
    /// Readonly prepared ordinary node configuration.
    Configuration(usize),
    /// Supplied struct arguments in source order, paired with declared field indices.
    Construct(Vec<(usize, Value)>),
    /// A field of an ordinary struct, addressed by its checked index.
    Field(Box<Value>, usize),
    /// Read the indexed prepared run-owned scalar entry.
    GlobalGet(usize),
    /// Lexical aggregate view: local id, prepared entry index, and write authority.
    BorrowedLocal(usize, usize, bool),
    /// Replace the indexed prepared entry with an owned scalar value.
    GlobalSet(usize, Box<Value>),
    /// Nullable presence test.
    IsPresent(Box<Value>),
    /// Payload extraction justified by frontend presence facts.
    Present(Box<Value>),
    /// A compile-time scalar literal.
    Literal(Literal),
    /// A provider-dependent recipe executed during ordered preparation.
    TemporalLiteral(hgl_source::TemporalLiteral),
    /// Cold prepared configuration binding; never a node-hook expression.
    Prepared(usize),
    /// Graph wiring marker containing the producing node index.
    Wire(usize),
    /// Port index and whether its declared parameter is signal.
    Input(usize, bool),
    /// Node cache index.
    Cache(usize),
    /// Backend-only typed owner hoisted by generator lowering.
    GeneratorLocal(usize),
    /// Local binding index.
    Local(usize),
    /// Writable owned scalar local binding index.
    MutableLocal(usize),
    /// Selected native signature index and checked arguments.
    Native(usize, Vec<Value>),
    /// Checked binary operator and operands.
    Binary(String, Box<Value>, Box<Value>),
    /// Checked unary operator or scalar conversion and operand.
    Unary(String, Box<Value>),
    /// Admitted metadata/capability operation and checked arguments.
    Query(String, Vec<Value>),
    /// The current node's output endpoint value.
    Output,
    /// Non-value injection marker; cannot be emitted as a payload.
    Capability,
    /// Absence of a payload; cannot be emitted as a payload.
    Void,
}
/// One constructor component in written evaluation order.
#[derive(Debug, Clone)]
pub enum DeltaEntry {
    /// Constant set member addition.
    Add(Value),
    /// Constant set member or map key removal.
    Remove(Value),
    /// Exact constant map key and its child publication payload.
    Keyed(Value, Value),
    /// Constant field/position and its exact child publication payload.
    Child(i64, Value),
}
impl DeltaEntry {
    /// Retained expressions in written key-before-payload order.
    pub fn operands(&self) -> impl Iterator<Item = &Value> {
        let (first, second) = match self {
            Self::Add(value) | Self::Remove(value) | Self::Child(_, value) => (value, None),
            Self::Keyed(key, value) => (key, Some(value)),
        };
        [Some(first), second].into_iter().flatten()
    }

    /// Writable retained expressions in key-before-payload order.
    pub fn operands_mut(&mut self) -> impl Iterator<Item = &mut Value> {
        let (first, second) = match self {
            Self::Add(value) | Self::Remove(value) | Self::Child(_, value) => (value, None),
            Self::Keyed(key, value) => (key, Some(value)),
        };
        [Some(first), second].into_iter().flatten()
    }
    /// Visit retained expressions in key-before-payload source order.
    pub fn try_map<E>(&self, mut check: impl FnMut(&Value) -> Result<Value, E>) -> Result<Self, E> {
        Ok(match self {
            Self::Add(value) => Self::Add(check(value)?),
            Self::Remove(value) => Self::Remove(check(value)?),
            Self::Child(index, value) => Self::Child(*index, check(value)?),
            Self::Keyed(key, value) => Self::Keyed(check(key)?, check(value)?),
        })
    }
}
/// Checked statements in a node lifecycle hook or handler.
#[derive(Debug, Clone)]
pub enum Statement {
    /// End evaluation without publishing a value.
    Exit,
    /// Local binding index and initializer.
    Let(usize, Value),
    /// Writable local binding index and owned initializer.
    Var(usize, Value),
    /// Bind a lexical aggregate view without retaining an owning copy.
    Borrow(usize, Value, bool),
    /// Publish a checked return value.
    Return(Value),
    /// Timed generator publication: time operand followed by payload operand.
    TimedYield(Value, Value),
    /// Checked runtime condition and loop body.
    While(Value, Vec<Self>),
    /// Return an ordinary value from a direct function invocation.
    Yield(Value),
    /// Evaluate an operation for its effect.
    Call(Value),
    /// Assign a checked value to its target.
    Assign(Value, Value),
    /// Iterate checked elements using a local binding index.
    For(usize, Value, Vec<Self>),
    /// Checked condition, true branch and false branch.
    If(Value, Vec<Self>, Vec<Self>),
}
/// One resolved runtime node and its source-defined behavior.
#[derive(Debug, Clone)]
pub struct Node {
    /// Qualified source name, combined with its plan index for registration.
    pub name: String,
    /// Port name, producing node index and resolved port type.
    pub inputs: Vec<(String, usize, Ty)>,
    /// Resolved output type, or void for a sink.
    pub result: Ty,
    /// Whether the node uses the source scheduler capability.
    pub alarm: bool,
    /// Checked generator source body, separate from ordinary lifecycle hooks.
    pub generator: Option<Vec<Statement>>,
    /// Checked start-hook statements.
    pub start: Vec<Statement>,
    /// Whether graph construction requires a provisioned run-wide scalar store.
    pub global_state: bool,
    /// Const key and exact scalar type for each prepared node access.
    pub globals: Vec<(String, Ty)>,
    /// Independently retained ordinary configuration initialized before hooks.
    pub configuration: Vec<Value>,
    /// Checked stop-hook statements.
    pub stop: Vec<Statement>,
    /// Cache initializers, indexed by cache expressions.
    pub caches: Vec<Literal>,
    /// Ordered handlers; absent guards use the existing input guard.
    pub handlers: Vec<(Option<Value>, Vec<Statement>)>,
}
/// One selected native signature used by the plan.
#[derive(Debug)]
pub struct Native {
    /// Qualified HGL declaration name for emitted documentation.
    pub name: String,
    /// Rust provider method name.
    pub method: String,
    /// Whether the provider returns a fallible result.
    pub throws: bool,
    /// Resolved argument types in declaration order.
    pub args: Vec<Ty>,
    /// Resolved result type.
    pub result: Ty,
}
/// A closed graph with all source checks and eval wiring complete.
#[derive(Debug, Default)]
pub struct Plan {
    /// Deterministic wiring operation failure reported during construction.
    pub construction_error: Option<String>,
    /// Nodes in construction order, addressed by index.
    pub nodes: Vec<Node>,
    /// Selected native signatures, addressed by index.
    pub natives: Vec<Native>,
    /// Selected source documentation retained in generated comments.
    pub docs: Vec<String>,
    /// Eval capture node index and observed scalar type, if any.
    pub output: Option<(usize, Ty)>,
    /// Ordinary recording key and exact retained list type, bound before start.
    pub recording: Option<(String, Ty)>,
    /// Dense eval input length, independent of expected output.
    pub input_length: usize,
}

impl Value {
    /// Whether an expression is already a closed ordinary constant.
    pub fn closed(&self) -> bool {
        match &self.kind {
            Kind::Delta(parts) => parts.iter().all(|part| part.operands().all(Value::closed)),
            Kind::Literal(_) | Kind::Void => true,
            Kind::List(items) => items.iter().all(Value::closed),
            Kind::Construct(fields) => fields.iter().all(|(_, value)| value.closed()),
            Kind::TemporalLiteral(_)
            | Kind::Prepared(_)
            | Kind::WiringFailure(_)
            | Kind::Index(..)
            | Kind::Length(_)
            | Kind::Push(..)
            | Kind::ValueCall(..)
            | Kind::Configuration(_)
            | Kind::Field(..)
            | Kind::GlobalGet(_)
            | Kind::BorrowedLocal(..)
            | Kind::GlobalSet(..)
            | Kind::IsPresent(_)
            | Kind::Present(_)
            | Kind::Wire(_)
            | Kind::Input(..)
            | Kind::Cache(_)
            | Kind::ObservedLocal(_)
            | Kind::Local(_)
            | Kind::MutableLocal(_)
            | Kind::Native(..)
            | Kind::Binary(..)
            | Kind::Unary(..)
            | Kind::Query(..)
            | Kind::Output
            | Kind::Capability
            | Kind::GeneratorLocal(_) => false,
        }
    }
}
