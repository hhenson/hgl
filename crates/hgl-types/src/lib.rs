//! The vocabulary every other crate shares: engine time, the scalar types,
//! the time-series types, and the node type.
//!
//! Data only. Nothing here allocates on a tick, and nothing here knows what a
//! graph is.

/// An instant on the UTC timeline, in microseconds. C++: hgraph's `DateTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineTime(i64);

/// A length of time, in microseconds. C++: hgraph's `TimeDelta`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineDelta(i64);

impl EngineTime {
    /// Before every time: the last modified time of something never modified,
    /// the schedule of a node that is not scheduled. The specification's
    /// `MIN_DT`. As in hgraph it is the epoch, 1970-01-01T00:00:00Z.
    pub const NEVER: Self = Self(0);

    /// The earliest a run may start: [`Self::NEVER`] plus one step. `MIN_ST`.
    pub const MIN_START: Self = Self(Self::NEVER.0 + EngineDelta::STEP.0);

    /// The latest a run may end: [`Self::FOREVER`] minus one step. `MAX_ET`.
    pub const MAX_END: Self = Self(Self::FOREVER.0 - EngineDelta::STEP.0);

    /// After every time: a graph's next scheduled time when nothing is
    /// scheduled. `MAX_DT`. As in hgraph it is 2300-01-01T00:00:00Z, which is
    /// 120,530 days after the epoch.
    pub const FOREVER: Self = Self(120_530 * 86_400 * 1_000_000);

    /// Unchecked: the range `NEVER..=FOREVER` is kept by `checked_add` and by
    /// the engine refusing a run configured outside it, not by this
    /// constructor.
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Microseconds since the epoch.
    pub const fn micros(self) -> i64 {
        self.0
    }

    /// `None` if the result is outside `NEVER..=FOREVER`, or the addition
    /// overflows. Judged on the result alone: a `self` outside the range may
    /// come back inside it.
    pub fn checked_add(self, delta: EngineDelta) -> Option<Self> {
        let micros = self.0.checked_add(delta.0)?;
        let sum = Self(micros);
        if Self::NEVER <= sum && sum <= Self::FOREVER {
            Some(sum)
        } else {
            None
        }
    }
}

impl EngineDelta {
    /// One microsecond: the smallest gap between two cycles. `MIN_TD`.
    pub const STEP: Self = Self(1);

    /// A length of that many microseconds; negative is backwards.
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// The length in microseconds.
    pub const fn micros(self) -> i64 {
        self.0
    }
}

/// A node's position in its graph's rank order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

/// The type of one scalar value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarType {
    /// A truth value: `bool`.
    Bool,
    /// A signed 64-bit integer: `i64`.
    I64,
    /// A 64-bit float: `f64`.
    F64,
}

/// A scalar whose type is known only at run time: a node's scalars, a case
/// table, a description. Never on the per-tick path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalarValue {
    /// A value of type [`ScalarType::Bool`].
    Bool(bool),
    /// A value of type [`ScalarType::I64`].
    I64(i64),
    /// A value of type [`ScalarType::F64`].
    F64(f64),
}

impl ScalarValue {
    /// The type this value is of.
    pub fn scalar_type(self) -> ScalarType {
        match self {
            Self::Bool(_) => ScalarType::Bool,
            Self::I64(_) => ScalarType::I64,
            Self::F64(_) => ScalarType::F64,
        }
    }
}

/// Recursive shape, independent of endpoint bindings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TsType {
    /// A scalar column.
    Ts(ScalarType),
    /// An i64 keyed dictionary.
    Dictionary(Box<TsType>),
    /// A designation to this shape.
    Reference(Box<TsType>),
    /// Fixed element shape and length.
    List(Box<TsType>, usize),
    /// Ordered, named fields.
    Bundle(Vec<(String, TsType)>),
}
impl TsType {
    /// Scalar column type. Aggregates have no scalar column.
    /// # Panics
    /// Called on a non-scalar shape.
    pub fn scalar(&self) -> ScalarType {
        let Self::Ts(t) = self else {
            unreachable!("not a scalar endpoint")
        };
        *t
    }
    /// Number of dense children.
    pub fn len(&self) -> usize {
        match self {
            Self::List(_, n) => *n,
            Self::Bundle(fields) => fields.len(),
            Self::Ts(_) | Self::Dictionary(_) | Self::Reference(_) => 0,
        }
    }
    /// Whether the shape has no dense children.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Fixed child shape at a wiring-time position.
    pub fn child(&self, position: usize) -> &Self {
        match self {
            Self::List(child, n) if position < *n => child,
            Self::Bundle(fields) => &fields[position].1,
            Self::Ts(_) | Self::Dictionary(_) | Self::Reference(_) | Self::List(_, _) => {
                unreachable!("not a fixed child")
            }
        }
    }
    /// Resolve a declared field while wiring.
    pub fn field(&self, name: &str) -> Option<usize> {
        if let Self::Bundle(fields) = self {
            fields.iter().position(|(n, _)| n == name)
        } else {
            None
        }
    }
    /// Whether this shape is a fixed collection.
    pub fn fixed(&self) -> bool {
        matches!(self, Self::List(..) | Self::Bundle(_))
    }
}

/// What part a node plays in its graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A source fed by events from outside the graph; real time only.
    PushSource,
    /// No inputs and an output: it ticks when it has scheduled itself.
    PullSource,
    /// Inputs and an output, or neither (as hgraph).
    Compute,
    /// Inputs and no output.
    Sink,
    /// A node that owns and runs child graphs.
    Nested,
}

/// Everything the runtime must know to make and run a node (specification:
/// Graph, Part 1, "Node type"). The first four fields are the signature.
///
/// `Default` is the specification's defaults — every input active and
/// required, no scheduler — so a node names only what differs:
/// `NodeType { name: "sum", inputs: .., output: .., ..NodeType::default() }`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeType {
    /// The name shared by every node of this type.
    pub name: &'static str,
    /// Number of reusable child graph templates required by this implementation.
    pub child_graphs: usize,
    /// The time-series inputs, by name.
    pub inputs: Vec<(&'static str, TsType)>,
    /// `None`: the node has no output.
    pub output: Option<TsType>,
    /// The fixed configuration values the node takes, by name.
    pub scalars: Vec<(&'static str, ScalarType)>,
    /// Which inputs schedule the node when notified. Positions in `inputs`.
    /// `None`: every input is active. `Some(vec![])`: none is.
    pub active_inputs: Option<Vec<usize>>,
    /// Which inputs must be valid for the node to be evaluated. Positions in
    /// `inputs`. `None`: every input must be valid. `Some(vec![])`: none
    /// need be.
    pub valid_inputs: Option<Vec<usize>>,
    /// Whether the node asks for a scheduler. One that does not has none.
    pub uses_scheduler: bool,
    /// Whether the node is scheduled for the start time when it starts.
    pub schedule_on_start: bool,
}

impl NodeType {
    /// From the signature: no inputs and an output is a pull source; inputs
    /// and an output, compute; inputs and no output, a sink; neither, compute
    /// (as hgraph). Owning child templates makes the node nested.
    pub fn kind(&self) -> NodeKind {
        if self.child_graphs > 0 {
            return NodeKind::Nested;
        }
        let has_inputs = !self.inputs.is_empty();
        let has_output = self.output.is_some();
        match (has_inputs, has_output) {
            (false, true) => NodeKind::PullSource,
            (true, false) => NodeKind::Sink,
            // Neither follows the oracle: hgraph's `static_node.h`, `node_kind`.
            (true, true) | (false, false) => NodeKind::Compute,
        }
    }
}
