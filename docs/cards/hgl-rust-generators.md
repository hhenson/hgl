# Card: hgl-rust-generators

Lower checked generator source bodies into typed Rust resume machines under
ADR0015 and its pinned operand-order clarification. Uses local `hgl-source`,
`hgl-rust-ir` and `hgl-rust-values`; budget 500 source lines. No third-party or
runtime execution dependencies. Source admission belongs to the frontend.

Public surface: `Generator::lower(body)`, `Generator::{fields, initialize,
start, evaluation}` return emitted storage, construction, restart and evaluation
fragments for a checked scalar-output generator. The generator stores a resume
position, the previous admitted target, one independently owned pending scalar,
and optional typed lexical locals. Structured if/while bodies lower to numbered blocks with explicit
successors; completing a block advances once, a future yield resumes at its
successor. Helper-call locals remain lexical and cannot collide with hoisted
source locals. Node restart clears all generator storage and requests its first
evaluation; it never runs the body in start.

Each reached yield evaluates its time operand, then its payload exactly once.
After both operands succeed, reject negative durations, resolve nonnegative
durations by checked addition, then require a target strictly greater than the
previous admitted target. Record the target before past/due/future handling;
ordering includes skipped absolute targets and survives resumption. Start resets
the predecessor. Past absolute targets skip publication, due targets publish
then continue, and future targets park an independently
owned payload before suspension. The existing source alarm checks scheduling
bounds; resumed publication moves that payload without reevaluating operands.
Local ownership uses the existing ordinary retention contract and prepared typed
representations. No runtime type dispatch, names, borrowing registry or shared
ownership enters the resume path.

Acceptance: source-generated debug/release execution for all eight scalar
outputs, nested loops/conditionals, hoisted shadowed locals, past/due/future
ordering, failures and duplicate timestamps, operand once-only effects, pending
payload independence, source completion and restart reset. Root integration
checks use only pinned spec cases and HGL implementation.

Structural generator pending owners use delta<result>, preserving exact
sparse data across suspension. Publishing a pending value applies its prepared
shape without reevaluating either operand or reconstructing held snapshots.

Immutable configuration projection yields additionally use hgl-rust-source-slots.
A generator with such paths stores generator_pending_slot:Option<ValueSlot<T>>
for delta<result>, alongside the original pending owner for arbitrary ordinary
expressions. Every reached yield still evaluates time then payload once. A parked
configuration slot is copied into prepared output storage on resumption without
extracting an owning payload or re-evaluating its path. Reset clears both pending
forms. Mixed control-flow paths retain the existing ordinary generator behavior.

Structural delta slots call their generated marker's apply_slot; complete atomic
or enum slots use the prepared atomic_from facade; scalar slots borrow their typed
scalar column and copy through the prepared scalar facade. The immutable node
configuration owns every referenced descendant for the generator's whole lifetime.
Acceptance includes emitted first/parked/repeated String publication and mixed
projected/native scalar execution in debug/release, counting allocations around
each complete Graph::evaluate call with no warm-up excluded.

Complete ordinary Set/Map pending payloads and prepared slots publish through atomic whole-value transport.
Prepared eval transport is selected as one complete path by
`hgl-rust-execution-proof::prepared(plan)`. Unproved plans retain existing ordinary
publication and recording behavior; no per-insertion fallback or hook replay is
introduced. Prepared allocation evidence applies only to the selected finite path.
Rolling generators retain delta<result>=V in their ordinary/configuration pending
forms. Publication preserves the declared rolling result context and copies the
arrival into its independent prepared ring. Statement emission carries an optional
result type through nested blocks; ordinary helper bodies have no temporal result.
