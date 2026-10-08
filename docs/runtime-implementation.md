# Runtime implementation

Status: implemented through the Rust API; the first scalar HGL compiler slice runs.

The prototype runs fixed TSL/TSB, TSD, REF and nested graphs together. Endpoint
identity belongs to the shared store; node rank belongs to a graph scope.
A child owns its state and scheduler. Its owner runs it only when notified or
due, and exposes its output without copying it.

The [type and wiring review](compiler/wiring-review.md) records the current
construction limits: exact ordered bundle shapes, limited REF compatibility,
no nominal bundles, and no generic or operator resolver. These are incomplete
parts of WIR-1–24, not alternative language rules.

Shapes compose recursively: `TS[bool/i64/f64]`, fixed TSL, named TSB fields,
`TSD[i64, child]` and REF. The description builder supports recursive shapes and child templates; the
compiler currently lowers the scalar const/debug bootstrap.
Growing TSL, key-set output ports and captured child errors remain pending. Independent root graphs can run sequentially in one
store; concurrent root clock domains are not implemented.

## Contract coverage

The [accepted observations](https://github.com/hhenson/hgraph_spec_audit/blob/main/archive/hgl/runtime/validation.md) remain the oracle.
These tests establish the following subset, not all 243 observations:

| Tests | Contract exercised |
|---|---|
| [Store](../crates/hgl-store/tests/dynamic.rs) | Membership versus publication, validity, sampled times, dictionary rebind/withdrawal, same-cycle restoration, next-cycle expiry, stale writers and detachment |
| [Combined trace](../crates/hgl-nested/tests/accepted.rs) | Keyed creation, REF routing, accumulated state, timer replacement, removal and recreation; repeated through another owning graph |
| [Timers and lifecycle](../crates/hgl-nested/tests/timers.rs) | Child-only deadlines, cancellation, siblings, coincident input/timer, failed construction/start/evaluation and bounded churn |
| [Allocation](../crates/hgl-nested/tests/no_alloc.rs) | Zero allocations over 10,000 warm steady cycles with REF switching, a child graph, attached TSD output and timer replacement |
| [Released members](../crates/hgl-store/tests/released_members.rs) | Parent removal notification/retention, slot reuse, late binding, same-cycle replacement isolation and REF-expiry detachment |
| [Child stop](../crates/hgl-nested/tests/start_stop.rs) | Unscheduled child start-hook stop propagation through owners, from root start or evaluation |
| [Invalidation](../crates/hgl-store/tests/invalidation.rs) | Silent repeated invalidation; retained parent timestamp and empty delta |
| [Bindings](../crates/hgl-bindings/tests/admission.rs) | Collection passivity and cancelled notifications from released child scopes |
| [Deadlines](../crates/hgl-deadlines/tests/model.rs) | 100,000 replacements/cancellations against an ordered model; reservations preserve live slots |

Removed children remain readable through their removal cycle. At the next
engine cycle their generations expire, even without another dictionary write.
A later insertion cannot revive a saved reference. Equal REF designation does
not sample the target again; target ticks and REF ticks remain separate.

## Validation

`cargo xtask ci` passes on macOS and the private Linux validation host,
including debug/release tests, clippy, docs, dependency checks and unchanged
existing line budgets. Local Windows had no Cargo; hosted Windows CI is
reported on the PR.

`python3 tools/dynamic_mutants.py` tests a disposable checkout, then tests the
restored baseline. Thirty-one compiled mutations fail their intended tests:
generation checking/wraparound, endpoint and child-view expiry, sample time, repeated following,
subscription detachment, collection activity, ancestor wakes, released-child
mailboxes, retained deadlines, duplicate child evaluation, failed-start stop,
graph restart, released-parent membership, same-key replacement isolation, and
both heap repair invariants, child-start stop propagation and repeated
invalidation. The active detachment test
was added after its mutation survived the passive trace. Scope release now
removes dictionary views before expiry; its mutation checks cover that removal
and the separate teardown when a REF expires while its target stays alive.

[Paired scalar measurements](../bench/results/2026-09-21-dynamic-foundation.md)
remain below the C++ ceiling but show the added metadata cost over P1. They
do not establish dynamic-case performance parity. A paired dynamic benchmark,
recursive compiler lowering and the wiring resolver remain required before calling this the
complete prototype.

## Fixed collection acceptance

All 44 accepted scenarios replay against Rust: 12,142 assertions, including
four real switch/map child graphs with timers and fresh state. Fixtures are
exported from the initial reasoning, recorded corrections and user rulings;
Python/C++ observations remain unchanged. Run `python3 tools/fixed_fixtures.py --check` to detect fixture drift. Known reference deviations remain in the
[comparison report](https://github.com/hhenson/hgraph_spec_audit/blob/main/runtime/validation/fixed/README.md).

Fixed children keep their handles across whole, assembled and empty bindings.
An assembled parent caches child validity and time; ordinary reads do not scan
for its timestamp. Owned collections keep publication validity until whole
invalidation. TSB observations retain every declared field; invalid children
read nil and deltas select valid modified children. Equal REF publication is
silent after the first tick. Structured designations are interned during
wiring and reused when switching; their arena lasts for the Store.

Tests also cover rejected shape/rank changes, compound subtree expiry,
same-cycle restoration, bounded endpoint/subscription churn and 10,000 warmed
nested ticks/REF switches with zero allocations. Eleven fixed-slice mutations
exercise duplicate REF ticks, unchanged-child resampling, reset, immediate
all_valid, subtree expiry, descendant rank, ancestor timestamps, compound dictionary sample time, stopped-writer
restoration, dead-endpoint insertion and premature scope reuse.

[Paired measurements](../bench/results/2026-09-22-fixed-collections.md) cover
native owned/assembled TSL[TSB] graphs and alternating whole REF routes.
They do not establish a performance bound for every dynamic nested graph.

Removed outputs from a stopped child scope remain readable through the removal
cycle but cannot be restored. Recreating the key allocates fresh storage owned
by the parent. Compound attachment takes a generation-checked `Reference`,
rejecting expired children even after their slots have been reused.

Scope slots also wait for the next cycle before reuse, preserving retained
references' original graph rank through the removal cycle.

Complete ordinary publication follows the pinned
[bounded structural value contract](../external/hgraph_spec/language/docs/design/structural-value-publication.md).
Prepared Tuple, concrete Struct, fixed List and Map observations reconcile child
validity and membership at return or own-output assignment. Sparse deltas preserve
omitted children. Empty/wholly invalid results, new invalid Map membership and
new growing-List reconciliation remain outside this slice. The previous retained
Tuple growing-List behavior remains admitted. Runtime native collection result
children require prepared ownership; unsupported paths diagnose before emission.
Shared complete-publication scenarios and recursive own controls measure all
first/repeated graph evaluations and recording with zero tick allocations.
