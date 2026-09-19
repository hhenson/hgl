# 0006 — What we need to do, and in what order

Status: sketch

The order of work for the shape in [0005](0005-code-shape.md). **The runtime
comes first**: it is the target, and nothing can be emitted for, or ported to,
a runtime that does not exist. It can be built and proven with no compiler at
all — graphs wired by hand-written Rust, checked against hgraph's own tests.

(An earlier draft ran a thin slice through a new Rust front end at the same
time. That put ~17k lines of compiler on the critical path for no gain in
confidence about the runtime, and it widened the brief, which asked for a new
runtime and a new *emitter*.)

Three rules from the evidence ([0003](0003-ai-codegen-evidence.md)) still set
the order within that: the oracle before the code, a thin slice before
anything widens, risky designs spiked before they are specified.

## The oracle

hgraph's runtime tests are `eval_node` cases: inputs per engine cycle in,
expected outputs per cycle out. There are ~2,540 `eval_node(` call sites in
the Python tests and ~1,520 in the C++ tests (2,182 `TEST_CASE`s). They are
runtime-level, need no compiler, and already exist — a far larger oracle than
the ~145 `assert eval` assertions in the `.hgl` corpus, which come back into
play with the emitter.

So the first piece of runtime to exist is the Rust equivalent of `eval_node`,
and each area is ported *tests first*: transcribe the hgraph cases for the
area, watch them fail, then build.

## How each crate gets built

This is the method under test, so it is stated once and then held constant.

1. **A contract card first**, one page, written by us, not generated:
   purpose; the public surface, listed; invariants as checkable laws; crates
   it may use; line budget; which hgraph tests and upstream doc sections are
   its oracle.
2. **The test structure second**, set up by hand: where cases live, how
   ticks are compared, how a failure is triaged. Agents extend a structure
   well; they do not invent a good one.
3. **Implementation by an agent** holding the card, the upstream sections and
   the failing cases — not the whole repository.
4. **Review by a fresh-context agent that sees only the diff** and the card,
   then by the owner using the checklist in
   [rust-practices](../guides/rust-practices.md).
5. **When something comes out wrong, fix what produced it** — the card, a
   lint, a budget, a test — and regenerate. A hand-patch hides the fault in
   the method.

Every review comment that could be a lint becomes one.

## Part A — the runtime, no compiler anywhere

| # | Step | Done when |
|---|---|---|
| A0 | **Spike the ownership model** (throwaway, outside the workspace): TSD of TS, a REF into it, a switch that tears a branch down; per-tick overhead against the reference | Positions 6 and 7 in 0005 are accepted, amended or replaced — with numbers |
| A1 | **Contracts for the three runtime seams**: `Node` + `NodeType`, the wiring API, the provider boundary — starting from the `.hgspec` draft | Three cards, each with laws that can be run |
| A2 | **Slice 1**: `value` (scalars), `ts` (TS only), `kernel` (graph, compute node, rank-ordered evaluation, simulation clock), `wiring` (builder + the `eval_node` harness). Nodes are hand-written Rust | hgraph's scalar arithmetic and comparison `eval_node` cases pass. Every runtime crate exists, tiny, under budget |
| A3 | **What a node can do**: state, scheduler and alarms, `start` / `stop`, activation and validity policy, passivate / activate, node errors and error outputs, pull sources and generators | The matching hgraph node and lifecycle cases pass |
| A4 | **Widen by time-series kind**, tests first each time: TSB and structs → fixed TSL → TSS → TSD → unbounded TSL → TSW → REF → SIGNAL; the remaining value kinds and the temporal scalars alongside | The matching cases pass per kind, including delta and validity behaviour |
| A5 | **Operators**: contracts, candidates, patterns, ranking, ambiguity (`hgl-resolve`), and the registry in `wiring`. Checked against selections recorded from hgraph | Overload selection agrees with hgraph on the recorded vectors |
| A6 | **Dynamic graphs**: switch, map over TSD and TSL, reduce; then mesh, feedback, try/except | hgraph's nested-graph cases pass, including teardown |
| A7 | **Real time**: the real-time clock, push sources and their queue policies, externally driven stepping | A thread drives a live graph; the simulation cases still pass |
| A8 | **The native operator set** needed by everything above, as `hgl-std` — arithmetic, comparison, collection primitives — and no more | Budget held; anything expressible in HGL is left for HGL |

## Part B — getting HGL onto it

Begun once A4 is solid; it does not wait for A7.

| # | Step | Done when |
|---|---|---|
| B0 | **Write by hand what the emitter should emit.** For a spread of corpus files (`midpoint`, `stateful-node`, `when-defaults`, `structural-types`, a conditional, a map) write the Rust a compiler *ought* to generate against the wiring API. This is the emitter's specification, and the first real test of whether the wiring API is pleasant to target | The hand-written versions pass the `.hgl` files' own `assert eval` expectations |
| B1 | **Decide the compiler route**, with a real target in hand. Two questions. *What does the compiler emit?* Either code that performs the wiring when run (what hgraph does today; the runtime must then supply and specify the whole wiring interface, type and operator resolution included), or a **stored graph description** the runtime loads and instantiates directly (wiring then lives in the compiler and the runtime tracks only the description — see the [runtime specification](../runtime_spec/graph.md)). *Where does the compiler live?* (a) a new emitter inside the existing C++ compiler — its emitter is already a pure IR → text pass; (b) the C++ compiler writes its IR as JSON and the emitter lives here, in Rust; (c) a Rust front end. Cheapest first; (c) only if the method is to be tested on a compiler too | Recorded in `decisions/` |
| B2 | **The emitter**, held to B0's output | Every `.hgl` corpus case gives the ticks the reference gives |
| B3 | **Natives and the standard library**: the Rust projection of `native fn` (upstream first — ADR 0008 target mappings), then the stdlib `.hgl` sources compiling for this runtime | The upstream stdlib tests pass here |
| B4 | **Scripted runs** (`hgl test`, `hgl run` without a build step): an interpreter over Graph IR, if still wanted | Parity with generated code on the corpus |
| B5 | **The outside world**: PyO3, then one adaptor (Kafka or Arrow) to prove "leverage existing frameworks" | A Python function runs as a node; a live source drives a graph |

Deliberately late or out: checkpointing, distributed graphs, services and
contexts, the profiler and observers, the REPL, an LSP. None is kernel.

## What would stop this

- **The spike loses.** If arenas-and-ids cost too much per tick, the fallback
  (pointers in one audited crate) gives back part of the reason Rust was
  chosen. Better to know at A0 than A6.
- **The wiring API is shaped by its tests, not its real client.** Built with
  no compiler in sight, it could turn out awkward to generate code for. B0 is
  the guard, and every hand-written node from A2 on should be written the way
  an emitter would write it — mechanical, no cleverness.
- **Test transcription is where the oracle leaks.** A wrongly transcribed
  expectation teaches the runtime the wrong behaviour. Transcribe
  mechanically, keep a pointer to the hgraph test each case came from, and
  spot-check a sample against the reference.
- **Budgets prove fictional.** If slice 1 cannot fit, correct the total once,
  in a decision — not a crate at a time.
- **The cards do not steer.** If agents given a card still produce the
  familiar mess, the experiment has its answer, and it is a useful one.

## Open questions

- Transcribe hgraph's tests into Rust test functions, or into a
  language-neutral case table (JSON/TOML) that a small Rust runner executes?
  The table survives a change of language and can be checked against the
  reference mechanically; Rust tests are quicker to write and to debug.
- Python or C++ tests as the source? The Python ones are terser and more
  numerous; the C++ ones exercise the runtime without the Python layer.
- Should the spike be kept as a benchmark once the kernel exists?
- If the compiler emits a stored graph description, what implements a node
  whose behaviour is an HGL body? Compiled code registered under a name the
  description refers to, or the body carried in the description and executed
  by the runtime. The second makes B4's interpreter part of the main route
  rather than an option. Either way, operator resolution (step A5) moves out
  of the runtime and into the compiler — unless wiring has to depend on
  values known only at launch (the constant parameters `hgl run` supplies),
  in which case some wiring must still happen where those values are.
