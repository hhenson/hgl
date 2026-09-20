# 0007 — Where hgraph's own documents disagree

Status: historical survey; current evidence is in the
[runtime evidence ledger](../runtime_spec/evidence.md).

Input to the [runtime specification](../runtime_spec/overview.md). Collected
2026-09-19 by reading hgraph's documents against each other and against the
C++ sources; nothing here was checked by running code. Paths are in the
hgraph repository. *SPEC* is `docs/source/specification/` (dated 2025-12-20,
describing the Python-era runtime); *DG* is `docs/source/developer_guide/`.

The rule the specification follows: DG and the RFCs override SPEC; a ruling
made for the C++ runtime overrides released Python behaviour. Each row is a
place that rule was applied, or could not be.

## Engine, graph, node

| Topic | One source says | Another says |
|---|---|---|
| Restart | SPEC 04 §2.1 state machine: Stopped → Started | Same file's table, and DG `architecture.rst`: stop is final (ruling 2026-07-04) |
| Initialise / dispose | SPEC 04: lifecycle hooks | Do not exist; edges are bound at construction |
| Engine, clock, scheduler objects | SPEC 04: separate Engine, Scheduler with a priority queue, Clock with `push_time` and `cycle_id` | DG: run state folds into the executor; the graph holds one schedule slot per node; the clock has evaluation time, now, cycle time, next cycle time |
| Evaluation guards | SPEC 04 §16.3: "trigger present" and "scheduler verification" | None: the schedule slot is the only gate, so a cancelled timer still evaluates its node once |
| Scheduling in the past | DG: invalid, throws | The node scheduler silently ignores `when <= now` once started |
| Real-time evaluation time | SPEC 04 §6.1: `now()` | DG `execution_layer.rst`: min(scheduled, max(previous + step, wall clock)) |
| End time | — | Exclusive; only the implementation and `execution_layer.rst` say so |
| After-evaluation callbacks | DG `architecture.rst`: registration order | `execution_layer.rst` and `executor.cpp`: last-in first-out |
| Initial tick | SPEC 06 §2.3: nodes get one | Nothing is scheduled by default |
| `CLOCK.now` | SPEC 06: the evaluation time | DG: the wall-clock estimate, distinct from evaluation time |
| Rank prefix | DG `graph_wiring.rst`: source nodes first | Only push sources are forced first |
| Push failure | SPEC 04 §16.1: failed messages re-queued with back-pressure | RFC 0027: back-pressure at admission only |
| Push queue options | SPEC 10: `batch`, `elide` | RFC 0027: `burst`, `conflate` |
| Injectables | SPEC 04 §13: `REPLAY_STATE`, and `CONTEXT` as an injectable | SPEC 02: `RECORDABLE_STATE`; a context is a time-series resolved at wiring |
| NodeError fields | SPEC 08 §7.6: exception type, message, traceback, input values | DG `error_handling.rst`: signature name, label, wiring path, message, stack trace, activation back-trace, additional context |

## Nested graphs

| Topic | SPEC says | DG `nested_graphs.rst` / `mesh.rst` says |
|---|---|---|
| Inputs to a nested graph | All become REF | Bound directly to the same upstream output; REF only by ordinary endpoint negotiation. No stub nodes |
| Component | A nested REF wrapper | Inlined at wiring; not a nested graph |
| Switch, old branch | Disposed | Stopped; destroyed later (at the next switch) |
| Reduce, non-associative | TSL only | A left fold over contiguous `TSD[int, E]`; zero used only when empty or singleton; reduces the valid subset |
| Mesh | Dependents read by REF; cycles handled with feedback | Forwarding nodes; settles within one cycle; a dependency cycle is a run-time error |

## Time-series

| Topic | One source says | Another says |
|---|---|---|
| `all_valid` | Commit message of 2026-08-09: recursive | Ruling of 2026-09-19: one level — the endpoint is valid and each immediate live child is valid. Deliberately differs from released Python |
| TSS / TSD validity | SPEC 05: "always valid" | DG and code: valid once it has ticked; an empty set that has ticked is valid |
| TSW validity | DG `plans_and_ops/time_series.rst`: "minimum-gated for both models" | Code, HGL and the user guide: valid on the first value; `all_valid` at minimum size or span |
| TSW delta | SPEC 05: a tuple | DG: the single value pushed this cycle |
| REF ticks | SPEC 05 §9.5: also when the target is modified; delta is the target's delta | DG: only when the reference is written or re-pointed; value and delta are the reference |
| TS → REF binding | SPEC: "non-peered with cached value" | RFC 0036: output-owned conversion, one hop |
| Unbounded TSL | DG line: "no removals today"; SPEC knows only fixed TSL | RFC 0031 (accepted): dense, shrinks by tail truncation |
| SIGNAL | Absent from SPEC 05 | DG: an input kind that binds to any output and sees only its ticks |
| TSB validity | Owned bundle: valid while its own stamp is set, even if its only child was invalidated | Un-peered input bundle: valid when any child is valid |
| TSD `added` / `removed` | Key *membership* | Code also tracks *published-value* deltas: a key is "added" when its child first becomes valid, "removed" when that child is invalidated though the key remains. Essentially undocumented. Owner's ruling 2026-09-19: *added* means the key was added and does not require the child to be valid — membership is the concept |
| Rank and references | DG `architecture.rst`: a reference's dynamic target "cannot be rank-ordered structurally" and is a sanctioned exception to the rank invariant | Owner's ruling 2026-09-19: reading through a reference MUST maintain rank order; binding to an output of higher rank (further downstream) is not allowed |
| Once per cycle | Python: each input keeps a notify time and drops a second notification at that time | C++: the guard is on the output — a write at a time not newer than the last returns before the observers are walked — and the node's schedule slot is idempotent. No per-input stamp. An input that subscribes between two writes of one cycle is not notified by the second. The specification follows C++ (TS-6) |
| Making an input active | Python: notifies the node if the source is valid and ticked in this cycle (or the input was sampled) | C++ and its authoring guide: "only changes the input's subscription state. It never schedules the node"; pinned by `test_ts_input.cpp`. The specification follows C++ (TS-8) |
| Bind at run time | Python: notifies if the node is started or starting | C++: notifies if the input was active, the view carries a real evaluation time, and the bind is observable; a wiring-time bind carries no time and so cannot notify. Two run-time forms: silent (an unchanged route migrated) and sampled |
| Invalidation | Often read as "a modification" | Owner's ruling 2026-09-19: modified is true only when last modified time equals evaluation time. Becoming invalid resets that time to *never*, so the time-series reads neither valid nor modified and its value is nil — but it *notifies*, and an active input bound to it schedules its node. In hgraph its parent collection is stamped and does read modified. Separately, a keyed input whose reference is withdrawn reads modified and not valid at once |

## Values

| Topic | One source says | Another says |
|---|---|---|
| `int` | SPEC 02: arbitrary precision | 64-bit |
| Scalar list | HGL: `zoned_time`, no `bytes` | hgraph: `bytes`, no `zoned_time`. Owner 2026-09-19: HGL is still in progress; `zoned_time` is new and hgraph is to support it |
| Value kinds | HGL names none of cyclic buffer, queue, `any` | hgraph has all three. Owner 2026-09-19: `any` is a value kind; cyclic buffer and queue exist to implement TSW and are not part of the concept |
| Immutability | hgraph docs: containers immutable by default, `Mutable` a separate schema axis | Owner 2026-09-19: an output may hold an efficient mutable value, but exposes it read-only; values are stable within a cycle and may change in the next; anything retained is retained by copy. The type-erased value/view wrappers exist to make that transparent |
| Homogeneous `tuple[T, ...]` | SPEC 02: a tuple | A dynamic list |
| `State` | User guide: "a single scalar slot", bundle state "planned" | Same guide: multi-field struct state is in use |
| Global context | User guide: per thread; wiring copies it | DG and header: one process-wide slot, bound at construction, copied when wiring ends |
| Civil time ordering | HGL roadmap: blocked | RFC 0002: granted |
| A struct containing itself | HGL: rejected by the compiler (`semantics/resolve.cpp`), listed as "not yet defined" in three docs | hgraph core: supported, through an owned one-pointer field (`recursive_bundle`, `recursive_bundles`). Owner 2026-09-19: it was meant to be in HGL. Proposed upstream as HGL ADR 0012 (branch `docs/hgl-recursive-struct-fields`, not merged) |

## What the documents themselves leave open

- DG `architecture.rst`: the graph description passed from wiring to
  construction; how nested graphs share clocks, schedulers and memory.
- DG `error_handling.rst`: error capture for nested, switch and mesh nodes; a
  reset policy for a child after an error.
- RFC 0027 is still *Proposed* (stages 1–3 implemented); RFC 0006 and 0007
  (window clear; scheduled eviction) are not accepted.
- HGL `language-model.md`: `i64` overflow and division, NaN comparison,
  string ordering, `valid` and `last_modified` before a first tick, the shape
  of `delta()`, clearing an optional struct field.
- Nothing says what a stored reference means after its output is destroyed.
