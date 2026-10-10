# hgl-rust-execution-proof

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `execution_proof` module of `hgl-rust` (`crates/hgl-rust/src/execution_proof.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

`prepared(plan)` selects one complete evaluation transport before hooks execute.
It requires known membership mutation counts, finite owning payload widths, and
source schedules bounded by retained replay configurations or checked finite
direct generator schedules. Exact constant induction loops and added-element loops compose with per-output widths. Opaque
iteration bounds and owning growth in loops are unproved. Opaque owning-return
calls are unproved except the bounded built-in scalar formatter. Nonclosed sparse
constructors require the supported direct-return lowering and complete key-path
and payload bounds; arbitrary owning locals do not acquire that proof.
Prepared bytes-from-list return recognition requires a scalar bytes result or
a rolling bytes arrival result, selecting the matching reserved destination.
Recursive statement checks retain the node result, while ordinary call bodies
use their own result. Signal inputs never establish this source proof.

The current replay schedule proof recognizes a single increasing index traversal
of one retained configuration list, with at most one yield per row. Unproved source
schedules retain the existing generic adapter. This decision evaluates no source
expression or user hook, changes no source admission, and never switches storage
mid-publication. Only proved plans prepare destinations, forward through prepared
slots and append to prepared recording storage. Unproved plans preserve the
previous dynamic execution and its existing allocation behavior; they carry no
claim of allocation-free ticks.

Nonclosed ordinary List/Set/Map construction has no admitted prepared owning capacity proof and selects the complete generic adapter. Harness-only Captured values never establish a hot storage proof.
At a return boundary, hgl-rust-direct-deltas::supported certifies the exact sparse
constructor lowering. The same expression at an arbitrary owning local boundary
continues to require its ordinary storage proof; constructor support does not
broaden all Delta expressions automatically.

`direct_arrivals(plan)` returns a checked sum of direct generator publications.
Loop-free sequences add yields and conditional branches take their maximum.
Recognized replay loops contribute through their retained configuration lengths;
other loops remain unproved. This analysis never evaluates conditions, payloads,
providers, or hooks. Capacity planning adds direct arrivals to the eval horizon
for recording and duration-window storage before any source runs.

The complete prepared adapter also requires an installation phase. A plan marked
ordinary_instantiation always retains generic publication: finite source bounds
alone do not imply that temporal destination slots have been reserved.

Loop-free scalar-child i64 Map mutations with known literal keys are prepared
when payload widths and schedules are proved. Insert/update/upsert/remove and
invalidate effects contribute to finite membership bounds; contains is read-only.
No dynamic key, growing payload, provider execution or opaque loop is added to
this proof. Allocation-counted complete-publication fixtures exercise the path.

The complete owning proof delegates text value-call argument storage to
value_calls. A used owning text formal has no finite helper slot preparation;
it cannot establish a zero-allocation transport proof.

`owning_argument(&Ty)` checks native helper argument storage recursively through
Tuple, Struct, nullable and atomic children, using the existing scalar owning
classification for leaves. This cold proof distinguishes fixed positional
arguments from text or collection descendants; it changes no runtime layout or
return/mutation proof. value_calls combines it with structured literal-use proof.

Invalidating a constant-key map child retains no child payload, including a
structural child; it uses the same finite membership domain as scalar invalidation.

May use scalars. Direct and pure-helper byte conversion can use finite prepared transport; owning constructor locals and unknown helper shapes do not claim this proof.
