# Card: hgl-proto-nodes

## Purpose

The handful of nodes the cases and the benchmarks need. They are written by
hand, **the way an emitter would write them**: one fixed recipe, no helper
that hides the recipe, no cleverness. They are the emitter's specification in
waiting, and the first test of whether the node interface reads well.

## May use

`hgl-types`, `hgl-store`, `hgl-kernel`, `hgl-describe`.

## The recipe

Every node is exactly this, in this order:

1. a struct holding its handles, its scalars, and its state;
2. `impl Node` — `eval`, and `start` / `stop` only if it needs them;
3. `impl Buildable` — `node_type()`, `build()`.

No macros, no generic node-over-operator, no shared base. Two nodes that
differ by one line are two nodes. `node_type()` writes the fields that differ
from the defaults and ends with `..NodeType::default()`.

## Surface

```rust
/// Register every node in this crate.
pub fn register_all(registry: &mut Registry) -> Result<(), BuildError>;

pub struct AddOne;            // "add_one":   in: TS<i64> -> TS<i64>        in + 1
pub struct Shift;             // "shift":     in: TS<i64>, delta: i64       in + delta
pub struct AddConst;          // "add_const": in: TS<i64>, k: i64           in + k
pub struct Sum;               // "sum":       lhs, rhs: TS<i64>             lhs + rhs
pub struct RunningSum;        // "running_sum": in: TS<i64>, state total    total += in
pub struct Constant41;        // "constant_41": no inputs, schedule on start -> 41
pub struct ConfiguredSource;  // "configured_source": value: i64, schedule on start -> value
pub struct Pulse;             // "pulse": count: i64; emits 0, 1, 2, ... one per cycle,
                              //          rescheduling itself one step ahead while more remain
pub struct Checksum;          // "checksum": sink; adds each input to a running total
impl Checksum {
    /// For a benchmark to check after the run, reached with `Graph::node`.
    /// The C++ baseline keeps the same two numbers in globals.
    pub fn total(&self) -> u64;
    pub fn evals(&self) -> u64;
}
```

`Sum` is `add` in the C++ baseline and `Pulse`, `AddOne`, `AddConst` and
`Checksum` match `bench/baselines/cpp/scenarios.cpp` node for node.

## Rules

NOD-5 (a node writes only its own output), INJ-3 (`RunningSum`'s total is
node state, a plain field, and persists from cycle to cycle; it is not
recordable state, which P1 does not have, so NOD-9 does not apply yet),
INJ-2.

*Revised after the build (2026-09-20): a field named for an input called
`in` is `input` (`in` is a Rust keyword; an emitter needs the same rule).
Arithmetic is plain `+` on `i64`, as the C++ is — no case or benchmark comes
near overflow, and what HGL's `int` does on overflow is for the language
specification to say. `Checksum` wraps its `u64` total, as C++ unsigned
arithmetic does.*

## Speed

`eval` bodies are straight-line: reads, arithmetic, one write. No allocation,
no formatting, no branching on anything but the data.

## Budget

300 lines. *(Revised 2026-09-20 by the card's author, not the builder: 250
was set before the node interface existed. Written to the recipe, nine
nodes came to 330, of which 108 were `NodeType` literals; with
`..NodeType::default()` the recipe's fixed cost is about 25 lines a node
before `eval` is written. 300 is nine such nodes, `register_all` and
`Checksum`'s accessors, with little room to spare.)*

## Done when

`register_all` succeeds; each node has a unit test that instantiates it alone
and checks its `node_type().kind()`; the P1 cases of `hgl-testkit` pass with
these nodes.

## Report back

Whether the recipe was pleasant or tedious to follow, and which part of it a
compiler would find hardest to emit. That is this card's real deliverable.
