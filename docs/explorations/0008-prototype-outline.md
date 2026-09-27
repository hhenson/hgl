# 0008 — Prototype runtime: outline

Status: sketch

An outline, not code. It says what a prototype of the runtime would consist
of, in what order it would be built, and how it would be judged. It is built
from the [runtime specification](https://github.com/hhenson/hgraph_spec/blob/main/runtime/overview.md), under the
speed rules of [0009](0009-designing-for-speed.md), and it merges the spike
and the first slice of [0006](0006-build-sequence.md).

## The idea

A narrow runtime, not a broad one. It implements the riskiest part of the
specification end to end, and is measured against hgraph:

- the simulation engine, the schedule and the evaluation cycle;
- node lifecycle, admission and the node scheduler;
- TS, a bundle input, a dictionary (TSD) and a reference (REF);
- one nested node that instantiates child graphs, binds them, and tears them
  down.

There is no compiler in it. Graphs are described by hand-written Rust, and
nodes are written by hand *the way generated code would write them*.

It exists to answer four questions, in this order of importance:

1. Does the data layout of 0009 hold up — is it as fast as claimed, and is
   the `unsafe` in it sound?
2. Is the node implementation interface something a compiler could emit for,
   and a person could read?
3. Can a contract card with rule numbers steer an agent to acceptable code?
4. How big is it really?

## What it looks like

| Crate | Owns | Uses | `unsafe` | Budget |
|---|---|---|---|---|
| `hgl-types` | Scalar and time-series types: identity, what a type's value and delta look like | — | no | 1.0k |
| `hgl-store` | Value columns, the endpoint table, handles, binding, notification | types | **yes — the only one** | 2.5k |
| `hgl-kernel` | Graph instance, schedule (bitset and heap), the cycle, node lifecycle and admission, node scheduler, the simulation engine, clock and engine control | types, store | no | 2.5k |
| `hgl-describe` | The graph description as plain data, a builder for writing one by hand, ranking, validation, instantiation | types, store, kernel | no | 1.5k |
| `hgl-testkit` | The `eval` harness, a replay source and a record sink, the counting allocator, case tables, benchmarks | all of the above | no | 1.2k |
| `hgl-proto-nodes` | A handful of nodes written as generated code would be: constant, add, a sampler, a keyed pass-through, a nested node that keeps one child graph per key | kernel, describe | no | 0.8k |

About 9.5k lines. 0005 guessed 13k for the same ground (`value` + `ts` +
`kernel` + part of `wiring`); the difference is scope, not optimism — no
windows, no sets, no real time.

## The three interfaces it has to get right

**1. The node implementation interface.** An implementation declares three
things: the type of its state; how to make one, given the node's scalars and
its handles; and start, eval and stop, each given that state and a context.

- Handles are typed — *input of `f64`*, *output of dictionary of `i64`* — and
  are resolved at instantiation and at binding, never in eval.
- The context is the whole of what eval may touch: reading an input through
  its handle (value, delta, valid, modified), writing the output, recordable
  state, and the injectables the node type asked for. Nothing else is
  reachable, which is NOD-5 and INJ-2 made structural.
- An implementation is registered under a **name**, and a description refers
  to it by that name (GRF-9). This is the smallest form of the stable
  identity a stored description needs.
- Hand-written nodes follow a fixed recipe — no cleverness — because the
  recipe is the emitter's specification in waiting.

**2. The description and its builder.** The description is the data of the
specification's Graph chapter, Part 1, and nothing more: node descriptions in
rank order, edges, child graphs with their bindings. It holds no closures and
no pointers, so it can be written out later without redesign.

The builder is the prototype's stand-in for wiring: add a node, connect an
output path to an input path, attach a child graph, finish. *Finish* ranks
the nodes — a topological sort, push sources first, ties broken by the order
of insertion — validates GRF-4 to GRF-10, and computes the storage layout
that makes instantiation one reservation.

**3. The test harness.** The equivalent of hgraph's `eval_node`: a sequence
per input, one element per cycle with a marker for "no tick", and the
expected sequence per output. Cycles run at the start time plus *i* smallest
steps, which is hgraph's alignment, so cases transcribe directly. It is built
from a replay pull source and a record sink — ordinary nodes — so it tests
the runtime through its own front door. Cases are data tables, each carrying
the path of the hgraph test it came from.

## Slices

Each slice starts from its failing cases and ends green in debug *and*
release, with its benchmarks recorded.

| # | Slice | Rules it has to satisfy | Done when |
|---|---|---|---|
| P0 | **The yardstick first.** The first baseline scenarios (tick, chain, wide chain) written against hgraph's C++ interface, tested there, and measured on the validation host. Counting allocator, the Rust half of each pair as a skeleton, the harness's shape, the first ~30 scalar cases transcribed | — | Baselines are recorded; cases fail for the right reason; a benchmark runs on an empty engine |
| P1 | **One value moving.** Store with two columns, TS, bundle input, notification; schedule, cycle, lifecycle, admission; simulation engine; constant, add, record | ENG-1–5, 9–12, 15 · GRF-1–3, 6–9, 11–18, 20 · NOD-1–7, 11, 21, 22 · TS-1–6, 8, 14, 21, 22 · VAL-1, 16, 17 · INJ-1–3, 8 | Scalar cases pass; zero allocations per cycle; tick, chain and wide chain within 5% of their baselines |
| P2 | **Time of its own.** Node scheduler, pull sources, clock and engine control, passive inputs, node errors | GRF-12–14 · NOD-8–10, 12–15, 19, 20 · INJ-4–7, 10, 11 · ENG-13–15 | Scheduler, sampling and error-output cases pass; an idle cycle costs the same at 10^2 and 10^5 nodes |
| P3 | **Things that come and go.** TSD with its key set, REF in both directions, bind / sampled bind / rebind / unbind | TS-7, 9, 11, 15–20 · GRF-10 | Dictionary and reference cases pass; dense, sparse and churning dictionary within 5% of their baselines (written and measured first); cost per cycle is flat in the number of keys |
| P4 | **Graphs that come and go.** Child descriptions, instantiate and tear down, boundary binding, schedule reaching the owner, deferred release; one keyed nested node to drive it | GRF-21–24 · the Part 1 child-graph rules | Nested cases pass, including teardown; nested-graph churn within 5% of its baseline; Miri is clean on the store |
| P5 | **Judgement.** Numbers against the reference; line counts against budget; what the cards did and did not steer | — | Decision records: the data layout, the node interface, and whether the method goes on |

## Oracle

hgraph's own `eval_node` cases (see 0006): scalar arithmetic and comparison
for P1, scheduler and sampling cases for P2, the dictionary, reference and
nested-graph cases for P3 and P4. They are transcribed, not reinterpreted,
and a sample of each batch is re-run on the reference to catch transcription
errors. Where the specification has deliberately departed from hgraph — a
dictionary's *added* means membership (TS-19); references keep rank order
(TS-20) — the case is written from the rule and says so.

## Gates

`cargo xtask ci`, plus: the allocation-free test; the doubling tests; Miri on
`hgl-store`; tests in debug and in release, because they check different
things (0009); a line budget per crate; for every rule in a slice, at least
one test that names it; and every benchmark scenario within 5% of its C++
baseline, measured on the validation host (0009).

Miri is a nightly component and the pinned stable does not carry it, so the
store is checked under a nightly installed beside the pin — built or fetched
when it is wanted, not held as a second pin. Nothing asks for it yet: P1's
store is safe Rust, and `unsafe` enters only if a benchmark misses
([0003](../decisions/0003-unsafe-confined-to-the-store.md)).

The gates that run the tests run under a cap on their address space, so that
a test which sizes an allocation from a computed value fails the gate instead
of taking the machine down — which one of them did. Building and running the
tests fits inside 8 GB; the cap is not put on the other gates, because a
tool's own appetite is not what it is for. macOS cannot set the limit at all,
so the cap holds on the validation host and not on the development machine: a
new test is run there first.

## Left out on purpose

Real time and push queues; sets, growing lists and windows; operator
resolution; record and replay beyond the harness; everything under
*Deferred* in the specification; Python; any compiler. None of them bears on
the four questions.

## Settled

- **Budgets**: within ±5% of the same scenario on hgraph's C++ runtime; the
  baseline scenario is written, tested and measured first
  ([decision 0002](../decisions/0002-performance-parity-with-cpp.md)).
- **Where it is measured**: the owner's private Linux validation host. It has
  CMake, Ninja, a current g++, an hgraph checkout, and — since 2026-09-19 —
  rustup with this repository's pinned toolchain and a checkout of this
  repository on which `cargo xtask ci` passes.
- **Where the C++ baselines live**: here, beside their Rust twins, built
  against an installed hgraph, so that a pair is versioned as one.
- **`unsafe`** is allowed in the store crate from the start, and nowhere else
  ([decision 0003](../decisions/0003-unsafe-confined-to-the-store.md)).
- **Dependencies** agreed in kind: a fast hasher, an inline small vector, a
  bitset, a benchmark harness. Each specific crate is still named once in
  `[workspace.dependencies]` when it is first needed.
- **Python is out of scope for now.** Baselines are not cross-checked against
  hgraph's Python benchmark pack, and nothing in the prototype plans for a
  Python bridge.

Nothing is outstanding before P0.

## Progress

- **P0, C++ half — done 2026-09-19.** `tick`, `chain` and `wide_chain` are
  written against hgraph's C++ interface (`bench/baselines/cpp/`), check
  their own results against closed forms, and are measured:
  288 ns, 6,507 ns and 62,568 ns per cycle, spread under 0.5%
  ([results](../../bench/results/2026-09-19-cpp-baselines.md)). The
  reference evaluates a node in about 64 ns. `cargo xtask bench` is the
  one measuring tool for both halves of a pair.
- **P0, Rust half — done 2026-09-19**, written directly rather than through
  the card-and-agent method: it is the hand-built structure that method
  needs to extend.
  - `crates/hgl-alloc-count`: a counting global allocator, per thread, with
    tests that show the pattern the tick path relies on (a buffer that keeps
    its capacity allocates nothing). The one `unsafe` outside the store.
  - `crates/hgl-testkit`: the case tables — 13 cases, 9 transcribed from
    hgraph's `test_eval_node.cpp` and 4 written from rules of the
    specification, each saying which — and the harness's entry point, which
    fails every case with "no engine" until P1. The tests that run the cases
    are ignored, with that reason, so the tree is green.
  - `bench/twin`: the Rust half of each pair. Same command line, same output
    line and same closed-form expectations as the C++ baseline, pinned to the
    figures that program produced. It reports `"ok":false` until there is an
    engine, and `cargo xtask bench` refuses to time a run that fails its own
    check.
  - Short of the outline: 13 cases, not ~30. hgraph's harness tests yield
    nine scalar cases; the rest of its scalar coverage is operator tests,
    which belong with operators.
- **P1 — done 2026-09-20.** Six contract cards
  ([docs/cards](../cards/README.md)): `hgl-types`, `hgl-store`, `hgl-kernel`,
  `hgl-describe`, `hgl-proto-nodes` and the testkit harness with the twins —
  2,450 lines budgeted. Three departures from this outline, each explained
  there: P1 uses no `unsafe` (index handles into plain vectors are safe and
  sufficient for `Copy` scalars; `unsafe` comes in only if a benchmark
  misses); a node can already reschedule itself, because every benchmark's
  source must; nodes are boxed until P4. `cargo xtask ci` now holds every
  crate to the `line-budget` it declares.
  - `hgl-types` — built, reviewed, 91 of 250 lines. The review found four
    wrong implementations that every test passed; since then a card lists
    its **mutants** and a builder shows each one failing before it reports.
  - `hgl-store` — built and reviewed, 320 of 500 lines, safe Rust. The
    builder killed the card's eight mutants; a fresh reviewer then found
    twelve more that passed every test, the worst deleting a whole column
    (a per-input "notified at" stamp). hgraph's C++ showed the column should
    not exist: once-per-cycle is kept on the output, and making an input
    active never schedules its node. The specification (TS-6, TS-8), the
    conflicts log and the card were corrected, the column removed, and all
    57 mutants now die. The build also found two holes in the card — `set`
    could not assert its writer, and the kernel could not ask whether an
    input is valid by id — both closed in the card first.
  - `hgl-kernel` — built, reviewed, fixed: 537 of 700 lines, 61 tests, 93
    mutants (the seven survivors are equivalent or unobservable in P1, each
    argued rather than patched). The
    review found the kernel running cycles hgraph does not: a node woken
    early by an input kept its old timer entry, so after replacing its
    request with a later one it ran an empty cycle at the old time. The
    specification implied the right answer without saying it; it now says
    that evaluating a node uses its entry, whatever woke it. Seven more
    mutants survived, one of them allocating on the tick path. All are now
    fixed and pinned, along with four lifecycle holes the review opened (a
    failed start left a time scheduled; the graph's time was unset until the
    first cycle; stopping a graph that never started made it unstartable; a
    failing eval left a pass half-done). Before the review: 70 mutants tried;
    six survived the first pass and were killed by new tests, one is
    equivalent (a request consumed at "now" or "at or before now" differs
    only if a due time is skipped, which ENG-4 forbids; C++ uses `==`), and
    two (`Starting`, `Stopping`) cannot be observed until nested graphs
    exist. The ready set became a tree of bitmaps, because a flat bitset
    is a scan. The scaling claim holds in debug; in release a cycle is about
    8 ns and memory layout moves it more than graph size, so it moves to the
    validation host. The likeliest place to lose against C++ is a dense
    graph: C++ walks every node with one compare; the kernel inserts and
    takes each ready node.
  - `hgl-describe` — built, reviewed, fixed: 484 of 500 lines, 88 mutants
    (one equivalent). The review found `finish` breaking ties differently
    from hgraph, which ranks first in, first out, seeded in insertion
    order: the same wiring would have given different node ids, start and
    stop order and evaluation order. Matching the reference also needs the
    edges sorted *before* ranking, which the builder found in the C++ and
    the card had not said. `register` now checks what the compiler cannot
    (positions in range, distinct input names); before, an out-of-range
    valid position panicked during instantiation and an out-of-range
    active position silently made nothing active. The check refused a
    malformed node type on its first run. A failing `build` still leaves
    ports in the store: the graph is all or nothing, the store not until
    P4. `Buildable::NAME` went at the same time — a node states its name
    once, in `node_type()`. The budget went from 450 to 500, set from the
    measurement: 450 held only by terse code (errors routed through
    one-line constructors, values unpacked in tuple `let`s), which this
    project does not want. The builder's view of writing a node: `build`
    is pleasant (`ports.input("lhs")?`, the type inferred from the field);
    `node_type()` is an eight-field literal; and what an emitter would find
    hardest is keeping three things in step that nothing checks at compile
    time — input names in `node_type()`, the same names as strings in
    `build()`, and input *positions* in `active_inputs` and `valid_inputs`.

  - `hgl-proto-nodes` — built, 289 of 300 lines (the budget was 250 until
    the interface existed; see its card). 45 mutants: 44 die, one is
    equivalent (`Sum` binding `lhs` and `rhs` the other way round —
    addition commutes). The builder's verdict on the recipe: `eval` is
    pleasant and matches the C++ line for line; the `Buildable` half is
    tedious — in `AddConst`, one of 31 lines is the arithmetic. Each port's
    name is written twice and its type three times, and only instantiation
    checks they agree. Hardest for an emitter: `node_type()`, where
    compile-time facts are restated as run-time data.
  - `hgl-testkit`'s harness and the twins — built: 407 of 650 lines in the
    testkit, 296 of 300 in `bench/twin`. `run` builds a case's graph from a
    `Replay` for each input, the node under test and a `Record` on its
    output, so a case is driven through the runtime's own front door. One
    implementation of each serves every P1 case, all of them `TS[int]`; the
    card's question of whether a `bool` needs its own pair is still open, and
    `Replay` refuses a `bool` tick rather than answer it quietly.
  - **A test of this slice took the owner's machine down.** `Record` sized
    the vector it notes cycles in from engine time, so a tick at a distant
    time asked for one element per step since the start — and engine time
    runs to 10^16 steps. Two of these test processes were the largest tasks on
    the machine when it ran out of memory and the kernel's watchdog panicked
    it. The defect then survived two green runs of the gate, because the
    kernel's ENG-4 debug assertion catches the path that reaches it — but the
    gate also runs the tests in release, where that assertion is gone. Reproduced on the validation host under a memory cap
    (one allocation of 16 GB), then bounded: a record is told how many cycles
    it may note and refuses a tick after them. The rule to carry forward is
    that nothing may be sized from a value engine time computes, and that a
    debug assertion does not protect the release run beside it.
  - **Measured on the validation host, 2026-09-20**
    ([results](../../bench/results/2026-09-20-p1-twins.md)). The three
    scenarios are 11.1×, 6.0× and 5.7× *faster* than the C++ baselines they
    had to come within 5% of: about 11 ns per evaluated node against the
    reference's 65 ns, with the same checksums and the same number of node
    evaluations on both sides. The baselines were re-measured in the same
    session on an idle machine and reproduced within 1%. `wide_chain` gains
    no less than `chain`, which answers the kernel's open worry: taking each
    ready node out of a tree of bitmaps costs no more per node at 961 nodes
    than at 102. The margin is a P1 margin — no dictionary, no reference, no
    nested graph, no real time — and says the layout of
    [0009](0009-designing-for-speed.md) has headroom, not that the finished
    runtime will keep it.

## Open questions

- **`..NodeType::default()` hides a new field.** The enums are closed so that
  anything new fails to compile until handled; with struct-update syntax, a
  field added to `NodeType` later takes its default silently in every node.
  For hand-written nodes the saving is worth it; an emitter could write
  every field.
- **Names an emitter must rule on.** A port called `in` cannot be a Rust
  field (`in` is a keyword); the prototype uses `input`. `uses_scheduler`
  has to be found by reading the body for `schedule_in`.
- **Integer overflow.** The prototype uses plain `+` on `i64`, as the C++
  does (where signed overflow is undefined, so no scenario relies on it). In
  Rust that panics in debug and wraps in release. HGL's `int` semantics on
  overflow are listed as open upstream; an emitter needs them.

- `NodeType` names active and valid inputs by position, and `Buildable`
  repeats the implementation's name (`NAME`) beside `node_type().name`. An
  emitter can keep them in step; a person writing a node by hand cannot be
  helped by the compiler. Names instead of positions, or a check at
  `register`, would close it — at the cost of an error variant.

- The cards name rules; should a rule's test be written by the same agent as
  the code it checks, or by a different one?
- A C++ baseline is checked only by its own tests (Python is out of scope).
  How much testing does a baseline need before its figure is trusted as a
  target?
- Does `hgl-describe` belong above the kernel, as here, or is instantiation
  the kernel's and only the builder separate?
