# Dynamic runtime slice

Status: implemented through the Rust API; HGL lowering remains pending.

The prototype can now run TSD, REF and nested graphs together. Endpoint
identity belongs to the shared store; node rank belongs to a graph scope.
A child owns its state and scheduler. Its owner runs it only when notified or
due, and exposes its output without copying it.

The admitted shapes are `TS[bool/i64/f64]`, `TSD[i64, TS[T]]`, and REF to either.
The existing description builder remains scalar-only. Compound children,
REF-valued dictionary children, key-set output ports and captured child errors
are outside this slice. Independent root graphs can run sequentially in one
store; concurrent root clock domains are not implemented.

## Contract coverage

The [accepted observations](runtime_spec/validation.md) remain the oracle.
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
restored baseline. Twenty compiled mutations fail their intended tests:
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
compound children and compiler/description integration remain required before
calling this the complete prototype.
