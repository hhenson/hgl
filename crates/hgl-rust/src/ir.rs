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
    /// Read the indexed prepared run-owned scalar entry.
    GlobalGet(usize),
    /// Replace the indexed prepared entry with an owned scalar value.
    GlobalSet(usize, Box<Value>),
    /// Owned nullable replay payload at the checked i64 index.
    ReplaySlot(Box<Value>),
    /// Nullable presence test.
    IsPresent(Box<Value>),
    /// Payload extraction justified by frontend presence facts.
    Present(Box<Value>),
    /// A compile-time scalar literal.
    Literal(Literal),
    /// Graph wiring marker containing the producing node index.
    Wire(usize),
    /// Port index and whether its declared parameter is signal.
    Input(usize, bool),
    /// Node cache index.
    Cache(usize),
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
/// Checked statements in a node lifecycle hook or handler.
#[derive(Debug)]
pub enum Statement {
    /// End evaluation without publishing a value.
    Exit,
    /// Local binding index and initializer.
    Let(usize, Value),
    /// Writable local binding index and owned initializer.
    Var(usize, Value),
    /// Publish a checked return value.
    Return(Value),
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
#[derive(Debug)]
pub struct Node {
    /// Qualified source name, combined with its plan index for registration.
    pub name: String,
    /// Port name, producing node index and resolved port type.
    pub inputs: Vec<(String, usize, Ty)>,
    /// Resolved output type, or void for a sink.
    pub result: Ty,
    /// Whether the node uses the source scheduler capability.
    pub alarm: bool,
    /// Checked start-hook statements.
    pub start: Vec<Statement>,
    /// Whether graph construction requires a provisioned run-wide scalar store.
    pub global_state: bool,
    /// Const key and exact scalar type for each prepared node access.
    pub globals: Vec<(String, Ty)>,
    /// Checked stop-hook statements.
    pub stop: Vec<Statement>,
    /// Optional admitted buffer capability name and scalar payload type.
    pub capability: Option<(String, Ty)>,
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
    /// Nodes in construction order, addressed by index.
    pub nodes: Vec<Node>,
    /// Selected native signatures, addressed by index.
    pub natives: Vec<Native>,
    /// Selected source documentation retained in generated comments.
    pub docs: Vec<String>,
    /// Eval capture node index and observed scalar type, if any.
    pub output: Option<(usize, Ty)>,
    /// Dense eval input length, independent of expected output.
    pub input_length: usize,
    /// Replay node indices and their configured dense input slots.
    pub replay_inputs: Vec<(usize, Vec<Option<Literal>>)>,
}
