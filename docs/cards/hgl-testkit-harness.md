# Card: hgl-testkit harness, and the twins

## Purpose

Make `hgl_testkit::run` real, and make `bench/twin` run its three scenarios.
Both already exist as skeletons that fail for the right reason; this card
replaces the failure with the engine.

## May use

`hgl-types`, `hgl-store`, `hgl-kernel`, `hgl-describe`; and, as
dev-dependencies and in `bench/twin` only, `hgl-proto-nodes` and
`hgl-alloc-count`.

## Surface

```rust
// hgl-testkit — the signature changes: the caller supplies the nodes.
pub fn run(case: &Case, registry: &Registry) -> Result<(), Failure>;

/// The two nodes the harness is made of. Ordinary nodes, registered and
/// instantiated like any other, so the harness tests the runtime through its
/// own front door.
pub struct Replay;   // "testkit.replay": a pull source that emits a fixed sequence, one
                     //   element per cycle, skipping `None`, rescheduling itself while
                     //   ticks remain
impl Replay { pub fn load(&mut self, ticks: &[Option<ScalarValue>]); }
pub struct Record;   // "testkit.record": a sink that notes, per cycle, what its input showed
impl Record { pub fn seen(&self) -> &[Option<ScalarValue>]; }

/// `hgl_testkit::cases::Scalar` becomes `hgl_types::ScalarValue`; the local
/// enum is deleted.
```

`run` builds, for one case: one `Replay` per input, the node under test, one
`Record` on its output; instantiates; runs the simulation from
`EngineTime::MIN_START`, ending after `case.cycles` cycles when it is set;
and compares cycle by cycle. A node with no output, or a case whose node is
not registered, is a `Failure`, not a panic.

A description stays plain data (GRF-1), so a sequence is not in it. After
`instantiate` and before `start`, `run` loads each `Replay` through
`Graph::node_mut`; after `stop` it reads each `Record` through `Graph::node`.
A node's id is its position in the finished description, found by its
implementation name and, for the replays, a `slot: i64` scalar. `Replay` and
`Record` are typed by a scalar too if one implementation cannot serve `bool`
and `i64` both: that is this card's to settle, and to report.

## The twins

`bench/twin/src/main.rs`: replace `run`'s error with the real thing — build
the scenario with `hgl-describe`'s `Builder` from `pulse`, `add_one`,
`add_const`, `sum`, `checksum`; time `run_simulation` only; read the checksum
and the evaluation count back. The command line, the output line and the
closed forms do not change.

## Rules

ENG-3 (the inclusive start and exclusive end, through `case.cycles`), GRF-1.

## Speed

`Replay` and `Record` keep their capacity; a run of the harness allocates at
instantiation and not per cycle. A record is told how many cycles it may note
— every cycle the case can reach, and one more — and refuses a tick after
them, so nothing is sized from engine time, which runs to 10^16 steps. The
twins' timed section contains nothing but `run_simulation`.

## Budget

300 lines added to `hgl-testkit`; `bench/twin` stays under 300 in all.

## Done when

- `first_slice_cases_pass` loses its `#[ignore]` and passes;
  `until_there_is_an_engine_every_case_fails_for_that_reason` is deleted;
  the P2 cases stay ignored.
- `hgl_twin tick|chain|wide_chain` report `"ok":true` with the baselines'
  checksums.
- A test runs the `chain` scenario for 1,000 cycles after warm-up under
  `hgl-alloc-count` and counts zero allocations.
- On the validation host, `cargo xtask bench` puts `tick`, `chain` and
  `wide_chain` within 5% of `bench/results/2026-09-19-cpp-baselines.md`, or
  the report says by how much they miss and where the time goes.

## Dense source-language eval

`evaluation::evaluate<T: Scalar>(GraphDescription, &mut Registry, output: u32,
input_length: usize) -> Result<Vec<Option<T>>, String>` appends a typed recorder
and runs a fresh simulation. The dense length is the later of the last input
cell and last output tick. `evaluation::compare(expected, observed)` requires
identical lengths and values, reporting the first differing cycle. Recorder
buffers and dense result construction allocate; they are test instrumentation,
not benchmark timing code. General timed/structural test results remain pending.
