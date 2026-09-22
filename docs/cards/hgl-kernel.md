# Card: hgl-kernel

## Purpose

What runs a graph: the node interface, the schedule, the evaluation cycle,
node lifecycle and admission, and the simulation engine. This card fixes what
a node author — or an emitter — writes.

## May use

`hgl-types`, `hgl-store`, `hgl-deadlines`.

## What a node looks like

This is the whole of a node, as a person or a compiler writes it. (How it is
constructed is [hgl-describe](hgl-describe.md)'s.)

```rust
struct Sum { lhs: In<i64>, rhs: In<i64>, out: Out<i64> }

impl Node for Sum {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.lhs) + ctx.get(self.rhs));
        Ok(())
    }
}
```

Read it as C++: `Sum` is a struct holding three small handles; `eval` is its
one virtual method; `ctx` is the only thing it can reach. By the time `eval`
is called the node was scheduled for this cycle and both inputs are valid, so
it does not ask.

## Surface

```rust
pub type NodeResult = Result<(), Box<NodeError>>;

/// A node's behaviour. `start` and `stop` default to doing nothing.
/// `Any` is what lets `Graph::node` hand a node back as its own type.
pub trait Node: std::any::Any {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult { Ok(()) }
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult;
    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult { Ok(()) }
}

/// Everything a node may touch while one of its hooks runs. Borrowed for the
/// call; a node cannot keep it.
pub struct Ctx<'a> { /* private */ }

impl Ctx<'_> {
    // inputs
    pub fn get<T: Scalar>(&self, input: In<T>) -> T;
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool;
    pub fn modified<T: Scalar>(&self, input: In<T>) -> bool;
    pub fn last_modified<T: Scalar>(&self, input: In<T>) -> EngineTime;
    pub fn set_active<T: Scalar>(&mut self, input: In<T>, active: bool);
    // the node's own output: written in eval only, readable in every hook
    pub fn set<T: Scalar>(&mut self, output: Out<T>, value: T);
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T>;
    // clock
    pub fn evaluation_time(&self) -> EngineTime;
    pub fn next_cycle_evaluation_time(&self) -> EngineTime;
    // scheduler, P1 form: one pending request, which a new one replaces
    pub fn schedule_in(&mut self, delay: EngineDelta) -> NodeResult;
    pub fn is_scheduled_now(&self) -> bool;
    // engine control
    pub fn request_stop(&mut self);
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeError { pub node: NodeId, pub label: String, pub phase: Phase, pub message: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Phase { Start, Eval, Stop }
impl NodeError { pub fn new(message: impl Into<String>) -> Box<Self>; }  // node, label, phase filled in by the graph

/// One node of a graph instance, ready to run. Built by hgl-describe.
pub struct NodeSlot {
    pub node: Box<dyn Node>,
    pub node_type: NodeType,
    pub label: String,
    /// Inputs that must be valid before `eval` is called, already resolved
    /// from `node_type.valid_inputs`.
    pub required: Vec<InputId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle { Instantiated, Starting, Started, Evaluating, Stopping, Stopped }

pub struct Graph { /* private: slots in rank order, the schedule, lifecycle */ }
impl Graph {
    pub fn new(label: String, slots: Vec<NodeSlot>) -> Self;
    pub fn lifecycle(&self) -> Lifecycle;
    /// After this the graph's time is one step before `now`, so the first
    /// cycle may be *at* the start time and a cycle before it is refused.
    pub fn start(&mut self, store: &mut Store, now: EngineTime) -> NodeResult;
    /// One cycle. `now` is later than the last cycle's time (ENG-2).
    pub fn evaluate(&mut self, store: &mut Store, now: EngineTime) -> NodeResult;
    /// `EngineTime::FOREVER` when nothing is scheduled.
    pub fn next_scheduled_time(&self) -> EngineTime;
    /// Stops every started node, in reverse; returns the first failure.
    pub fn stop(&mut self, store: &mut Store, now: EngineTime) -> NodeResult;
    pub fn stop_requested(&self) -> bool;
    /// The node at `id`, if it is an `N`. C++: `dynamic_cast<N*>`. For tests
    /// and tools, outside a cycle: a harness loads a fixture into a node
    /// before `start` and reads what a sink gathered after `stop`. Nothing on
    /// the tick path calls these.
    pub fn node<N: Node>(&self, id: NodeId) -> Option<&N>;
    pub fn node_mut<N: Node>(&mut self, id: NodeId) -> Option<&mut N>;
}

pub struct RunConfig { pub start_time: EngineTime, pub end_time: EngineTime }
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError { BadTimes, Node(Box<NodeError>) }

/// Simulation: start, cycles at each next scheduled time before the end time,
/// stop. Returns the number of cycles run.
pub fn run_simulation(graph: &mut Graph, store: &mut Store, config: &RunConfig) -> Result<u64, EngineError>;
```

## Inside

- **The schedule** is a ready set over ranks for the current cycle and a
  min-heap of `(time, node)` for later, indexed by node for replacement and
  cancellation. It retains one deadline per node. It implements the
  store's `Wake`, idempotently. Nothing scans every node: the ready set is a
  64-way tree of bitmaps, so finding the lowest ready rank costs one word per
  level (three levels up to 262,144 nodes), not one per 64 nodes.
- **The cycle** (specification: Graph, "The evaluation cycle"): move what is
  due from the heap to the bitset; take set bits in ascending rank; for each,
  consume a request due now, check `required` inputs are valid, call `eval`,
  then re-arm from the node's pending request. A node that was scheduled but
  not admitted still consumes its request and is re-armed (Node, "The
  scheduler"). A bit set during the pass for a higher rank is reached by the
  same pass.
- **A visit uses the node's entry, whatever woke it** (GRF-14). After the
  visit the node's entry is its pending request — earlier *or later* than
  the entry it had. The deadline is replaced only when it changes.
  `entry_at`, `request` and `child_request` are per-node arrays. Between
  cycles the entry is the earlier live request, except a node scheduled on
  start, which holds the start time until its first visit. An input's wake
  only sets the ready bit; the entry is reconciled by the visit that wake
  brings. Nothing can read an entry mid-pass, so that is the specification's
  "the entry becomes *t*", arrived at lazily.
- **A pass cut short by a failing eval** gives up the cycle for the nodes it
  never reached: their ready bits are dropped and a request due now is used
  up, so no past time can reach the heap. (hgraph leaves its cursor and
  schedule as they are, which in its model resumes the cycle. P1 ends the
  run on a failure, so nothing can tell the difference; P2's captured errors
  will have to choose.)
- **`Ctx`** knows which node it was made for. `Ctx::set` passes that node to
  the store as the `writer`, and the graph's schedule as the `Wake`; a node
  author sees neither.
- **Start** in rank order; a node whose start fails is not stopped, the ones
  before it are, in reverse. **Stop** in reverse, every started node once,
  first failure returned. At the end of a simulation, stop reads the last
  cycle's time, or the start time if no cycle ran (as hgraph's executor).
- **A graph's own error** (started twice, GRF-20) is a `NodeError` with
  `NodeId(0)`, the graph's label and a message naming the rule. P1 has no
  better place for it; `EngineError` gains a graph variant when one is
  needed.
- **`Starting` and `Stopping`** cannot be observed in P1: no hook can reach
  the graph. They are for a nested graph's owner (P3); until then no test can
  pin them.

*Revised 2026-09-19, before the first build finished: `Graph::node` and
`node_mut` were missing. Without them a sink's result could leave the graph
only through a global (what the C++ baseline does) or shared ownership, which
the tick path bans. A description stays plain data; what a test must hand to
or take from one node goes through the graph, by type, outside a cycle.*

## Rules

ENG-1, 2, 3, 4, 5, 9, 10, 11, 15 · GRF-11, 12, 13 (debug assertion), 14, 15,
16, 17, 18, 20 · NOD-1, 2, 3, 4, 5, 11, 12, 14, 22 · INJ-2, 3, 6, 7, 8, 10.
NOD-19 (captured errors) is P2: in P1 a failing `eval` ends the run.

## Speed

- A cycle costs what is ready: no loop over all nodes, in `evaluate` or in
  `next_scheduled_time`.
- One indirect call per evaluated node. No allocation in `evaluate` once the
  heap and bitset have reached their size; `NodeError` allocates, and only on
  failure.
- `Ctx` methods are `#[inline]` forwards to the store.

## Budget

700 lines.

## Done when

Unit tests, each naming its rule, with nodes defined in the tests: rank-order
evaluation within one cycle; a node woken twice is evaluated once; a node
whose required input is not valid is skipped; `schedule_in` re-arms and is
consumed; scheduling into the past fails; start failure rolls back in
reverse without stopping the failed node; every started node is stopped once;
the end time is exclusive; `request_stop` lets the cycle finish; 100,000
cycles of a three-node graph make zero allocations after warm-up; a cycle
with one ready node costs the same in a graph of 100 nodes and of 100,000
(within 20%, median of paired runs — steady in debug; in release a cycle is
about 8 ns and memory layout moves it more than graph size does, so the
release form of this claim belongs to a benchmark on the validation host).

## Known limits

- **Stale heap entries** are dropped only when they surface. A node whose
  request moves every cycle — a debounce, re-armed on each input tick —
  leaves one stale entry per move until time passes it, so the heap can grow
  to (delay ÷ tick interval) entries and allocate while it does. An indexed
  heap (one entry per node, moved in place) bounds it; it waits for a
  benchmark or a real graph that needs it. A node whose request does not
  change pushes nothing.
- **Reading another node's output** is not caught, even in debug: the
  store's `output_value` does not know who reads. `set` catches the write
  (TS-21).
- **INJ-2** is held only for the scheduler; `NodeType` names no other
  injectable, so every node can read the clock and request a stop.

*Revised after review (2026-09-20): an input wake did not use up a node's
entry, so a node that replaced its request with a later one ran an empty
extra cycle at the old time — neither the specification's intent nor what
hgraph does (graph.cpp: an input wake overwrites the entry; node.cpp: the
request is written back after eval). The specification now says so
outright. Also: a failed start leaves nothing scheduled; the evaluation time
is the start time once started; stopping a graph that never started does
nothing; a failing eval clears the ready set.*

*Revised after the build (2026-09-19): `evaluate`'s time is "later than", as
ENG-2 says; the ready set is a tree of bitmaps, because a flat bitset is
scanned; the not-admitted re-arm, the stop time, the graph's own error and
the unobservable lifecycle states were unstated; the scaling claim is split
between debug and the validation host.*

## Mutants

Each must make a test fail.

- Nodes are evaluated in the order they were woken, not in rank order.
- A node woken twice in a cycle is evaluated twice.
- The required-input check is skipped.
- The end time is inclusive.
- Nodes are stopped in rank order instead of reverse.
- The node whose start failed is stopped.
- `schedule_in` re-arms but is never consumed (the node runs every cycle for ever).
- A pending request is consumed but never written to the graph schedule.
- Scheduling into the past is accepted.
- `request_stop` ends the cycle at once instead of after it.
- `next_scheduled_time` returns a stale heap entry — or drops only the first
  of several stale entries.
- A node scheduled but not admitted does not consume its request. (That it
  does not *write* its next one is equivalent in P1, which holds one request:
  the one consumed was the only one. It becomes a mutant in P2.)
- The schedule is not idempotent: a node woken twice is queued twice.
- `Ctx::modified` reads `valid`; `Ctx::last_modified` reads the evaluation
  time.
- A node woken early by an input keeps its old entry, and runs again at it
  after replacing its request with a later one.
- A heap entry is pushed on every wake, or when the new time equals the old
  (allocation on the tick path).
- A stale entry lying above the node's own later live entry is reported as
  the next scheduled time.
- A stop failure during a start's rollback replaces the start failure.
- GRF-13's assertion misses a node that wakes itself.
- The graph's own error (GRF-20) does not carry its label and `NodeId(0)`.
- A failed start leaves a time scheduled.
- The graph's time is not set by `start`, or is set to the start time
  itself (so the first cycle is refused).
- `stop` on a graph that never started marks it stopped.
- A pass cut short by a failure leaves a ready bit set or a due request
  unconsumed.

## Dynamic slice

Status: implementation contract. Existing budget stays.
Graph adds `scope` and enters/restores that scope for every hook. Each cycle
expires removed endpoints before evaluation and drains foreign notifications
into the local rank schedule. Stop cancels its schedule.

Ctx adds read-only `store`, `sample`, `set_reference`, `invalidate`,
`get_or_create`, `attach`, `remove`, `create_child`, `evaluate_child`,
`stop_child`, `take_child`, and `schedule_children`. Child factories return a
Graph and a boundary handle using the shared Store. Child scopes are created
before allocating ports and retired on construction/start failure or stop.
Internal child deadlines are independent of the owner's scheduler request.
Errors preserve the failing child hook and include the owner path.

`Graph::scope(&self) -> ScopeId` exposes graph identity. New `Ctx` methods:

```rust
fn store(&self) -> &Store;
fn sample(&mut self, input: InputId, r: Reference) -> NodeResult;
fn set_reference(&mut self, output: OutputId, r: Reference) -> NodeResult;
fn invalidate(&mut self, output: OutputId);
fn get_or_create<T: Scalar>(&mut self, dict: DictOut<T>, key: i64) -> Out<T>;
fn attach<T: Scalar>(&mut self, dict: DictOut<T>, key: i64, child: Out<T>) -> NodeResult;
fn remove<T: Scalar>(&mut self, dict: DictOut<T>, key: i64);
fn create_child<T>( &mut self, build: impl FnOnce(&mut Store) -> Result<(Graph, T), Box<NodeError>>) -> Result<(Graph, T), Box<NodeError>>;
fn evaluate_child(&mut self, graph: &mut Graph) -> NodeResult;
fn stop_child(&mut self, graph: &mut Graph) -> NodeResult;
fn take_child(&mut self) -> Option<ScopeId>;
fn schedule_children(&mut self, time: EngineTime);
```

A child stop request propagates to its owner after successful start as well as
after evaluation, even when the child has no scheduled evaluation.

Fixed collection mutation uses the existing ownership and cycle checks:
`Ctx::get_or_create_shaped(OutputId, i64) -> OutputId`,
`attach_shaped(OutputId, i64, Reference) -> NodeResult` and
`remove_shaped(OutputId, i64)`. New child graphs may return any shaped output;
fixed leaf handles are projected during construction and use ordinary `set`.
