# Card: hgl-types

## Purpose

The vocabulary every other crate shares: engine time, the scalar types, the
time-series types, the node type, and hook failure data. Successful primitive
access needs no allocation; creating an error message can allocate. No graph
execution lives here.

## May use

Nothing.

Shared hook failure data (`Phase`, `NodeError`, `NodeResult`) lives here and is
re-exported by hgl-kernel. Error construction may allocate; successful scalar
paths do not. `NodeType::validate_metadata()` checks duplicate input names,
active/valid input positions and global-state declaration consistency.
`uses_global_state: bool` requests provisioning;
`global_entries: Vec<(&'static str, ScalarType)>` declares exact construction
bindings, including the requirements of retained child templates.

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
pub enum ScalarType { Bool, I64, F64, Text, Date, Time, DateTime, Duration }

/// A scalar whose type is known only at run time: a node's scalars, a case
/// table, a description. Never on the per-tick path.
#[derive(Debug, Clone, PartialEq)]
pub enum ScalarValue {
    Bool(bool), I64(i64), F64(f64), Text(String), Date(Date), Time(Time),
    DateTime(EngineTime), Duration(EngineDelta),
}
impl ScalarValue { pub fn scalar_type(&self) -> ScalarType; }

/// Recursive, closed shape vocabulary shared by descriptions and endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TsType {
    Ts(ScalarType), Dictionary(Box<TsType>), Set(ScalarType), Reference(Box<TsType>),
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
    pub uses_global_state: bool,
    pub global_entries: Vec<(&'static str, ScalarType)>,
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
`Bundle` names its fields, not the bundle: nominal bundle identity is absent.
Derived equality describes the exact ordered shape; it is not WIR-15 matching.
The [wiring review](../compiler/wiring-review.md) separates that relation from
WIR-7 inference and from the binding plan needed to connect equivalent shapes.
`NodeType::child_graphs: usize` declares the required template count; nonzero
classifies the node as Nested. Existing budgets are unchanged.

`Date(pub i64)` stores epoch-relative days; `Time(pub i64)` stores microseconds
after midnight. Both derive Default, Clone, Copy, equality, ordering and Hash.
EngineTime/EngineDelta also derive Default (zero). `TsType::member() ->
Option<&TsType>` returns dictionary child shape or a boolean set occupancy
marker. Set keys currently use the runtime's i64 membership table; compiler
lowering admits bool/i64 elements only.

`OrdinaryType::{Scalar(ScalarType), Struct(&'static str,
Vec<(&'static str, OrdinaryType)>)}` describes a finite ordinary value with
canonical nominal identity and required named fields. `From<ScalarType>` wraps
primitive requirements. `NodeType::global_entries` uses this exact ordinary type;
access permissions are not part of type identity.

`NodeResult<T = ()>` also names typed capability results with the same translated
node error; existing hook results retain their unit default.

`OrdinaryType::List(Box<OrdinaryType>, Option<usize>)` includes exact recursive
element identity and fixed length; None is unbounded. Access authority and current
runtime length do not change the bound type.

Atomic publication storage adds `TsType::Atomic(OrdinaryType)`. It has no
structural children and retains the payload's exact canonical ordinary identity.
`OrdinaryType::Tuple(Vec<OrdinaryType>)` represents positional ordinary values;
OrdinaryType derives Hash alongside equality. Scalar atomic source spellings
normalize before runtime lowering and continue to use `TsType::Ts`.

## Temporal scalar preparation

Temporal scalar data now lives in hgl-time-values and is re-exported here.
ScalarType and ScalarValue add CivilDateTime, TimeZone, ZonedDateTime and ZonedTime with
the corresponding concrete data types. May use hgl-time-values; the existing
250-line budget is unchanged. See hgl-time-values.md for ownership and identity.

OrdinaryType::Enum(&'static str) describes a declared enum by its canonical
module-qualified identity. Its physical value is a checked assigned i64; enum
identity is never interchangeable with an ordinary integer or another enum.

KeyedDictionary(OrdinaryType, Box<TsType>) and KeyedSet(OrdinaryType) retain
exact scalar/enum key identity for prepared finite collections. Dictionary
remains the i64 compatibility shape; Set remains built-in scalar membership.

`OrdinaryType::OptionalField(Box<OrdinaryType>)` is an internal field-presence
descriptor, distinct from its present payload and required fields. It is never
an HGL annotation or a whole temporal-null publication type.

OrdinaryType::RecursiveReference(&'static str) is internal schema metadata for an
exact fully applied nominal edge. Concrete root schemas retain their existing
Struct identity and finite field descriptions; recursive targets never expand
while forming schema metadata. It adds no source reference or nullable type.
