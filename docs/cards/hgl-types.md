# Card: hgl-types

## Purpose

The vocabulary every other crate shares: engine time, the scalar types, the
time-series types, and the node type. Data only. Nothing here allocates on a
tick, and nothing here knows what a graph is.

## May use

Nothing.

## Surface

```rust
/// An instant on the UTC timeline, in microseconds. C++: hgraph's DateTime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineTime(i64);

/// A length of time, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineDelta(i64);

impl EngineTime {
    pub const NEVER: Self;      // before every time; "not modified", "not scheduled"
    pub const MIN_START: Self;  // NEVER + STEP
    pub const MAX_END: Self;    // FOREVER - STEP
    pub const FOREVER: Self;    // after every time; "nothing scheduled"
    /// Unchecked: the range NEVER..=FOREVER is kept by `checked_add` and by the
    /// engine refusing a run configured outside it, not by this constructor.
    pub const fn from_micros(micros: i64) -> Self;
    pub const fn micros(self) -> i64;
    /// `None` if the result is outside NEVER..=FOREVER, or the addition overflows.
    /// Judged on the result alone: a `self` outside the range may come back inside it.
    pub fn checked_add(self, delta: EngineDelta) -> Option<Self>;
}
impl EngineDelta {
    pub const STEP: Self;       // one microsecond: the smallest gap between cycles
    pub const fn from_micros(micros: i64) -> Self;
    pub const fn micros(self) -> i64;
}

/// A node's position in its graph's rank order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarType { Bool, I64, F64 }

/// A scalar whose type is known only at run time: a node's scalars, a case
/// table, a description. Never on the per-tick path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalarValue { Bool(bool), I64(i64), F64(f64) }
impl ScalarValue { pub fn scalar_type(self) -> ScalarType; }

/// Recursive, closed shape vocabulary shared by descriptions and endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TsType {
    Ts(ScalarType), Dictionary(Box<TsType>), Reference(Box<TsType>),
    List(Box<TsType>, usize), Bundle(Vec<(String, TsType)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind { PushSource, PullSource, Compute, Sink, Nested }

/// Everything the runtime must know to make and run a node (specification:
/// Graph, Part 1, "Node type"). The first four fields are the signature.
/// `Default` is the specification's defaults (every input active and
/// required, no scheduler), so a node writes `..NodeType::default()` and
/// names only what differs — C++: a designated initializer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodeType {
    pub name: &'static str,
    pub child_graphs: usize,
    pub inputs: Vec<(&'static str, TsType)>,
    pub output: Option<TsType>,
    pub scalars: Vec<(&'static str, ScalarType)>,
    /// Positions in `inputs`. `None`: every input is active. `Some(vec![])`: none is.
    pub active_inputs: Option<Vec<usize>>,
    /// Positions in `inputs`. `None`: every input must be valid. `Some(vec![])`: none need be.
    pub valid_inputs: Option<Vec<usize>>,
    pub uses_scheduler: bool,
    pub schedule_on_start: bool,
}
impl NodeType {
    /// From the signature: no inputs and an output is a pull source; inputs
    /// and an output, compute; inputs and no output, a sink; neither, compute
    /// (as hgraph). Child templates make the owner `Nested`.
    pub fn kind(&self) -> NodeKind;
}
```

## Rules

ENG-16 (the four named times and the smallest step; tests cite `ENG-16`). VAL-3 is not needed yet:
P1 has no named types.

*Revised 2026-09-20: `NodeType` derives `Default`. Every node written by
hand spelled out all eight fields; two builders independently named it the
most tedious part of writing a node.*

*Revised after the first build (2026-09-19): the no-input, no-output shape,
what the input positions index, that `from_micros` is unchecked, and a
numbered rule for the named times were all missing. `NEVER` is the epoch and
`FOREVER` is 2300-01-01, in microseconds, as in hgraph.*

## Speed

`EngineTime`, `EngineDelta`, `NodeId`, `ScalarType` are `Copy` and one word.
Comparing two times is one integer compare. `ScalarValue` and `NodeType` are
for instantiation and tests; nothing reads them during a tick.

## Budget

250 lines.

## Done when

Tests cover: the ordering `NEVER < MIN_START < MAX_END < FOREVER`;
`checked_add` refusing to leave the range; `NodeType::kind` for each
signature shape.

## Recursive descriptions

`TsType` is the single recursive shape vocabulary: `Ts(ScalarType)`,
`List(Box<TsType>, usize)`, `Bundle(Vec<(String, TsType)>)`,
`Dictionary(Box<TsType>)` (i64 keys), and `Reference(Box<TsType>)`. It exposes
`scalar`, `len`, `is_empty`, `child`, `field`, `fixed`; these are the existing
endpoint-shape operations. `hgl-endpoints::Kind` re-exports this type.
`NodeType::child_graphs: usize` declares the required template count; nonzero
classifies the node as Nested. Existing budgets are unchanged.
