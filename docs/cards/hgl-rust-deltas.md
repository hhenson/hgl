# Card: hgl-rust-deltas

Emit statically specialized ordinary structural delta layouts and endpoint
application/extraction from checked IR. Uses hgl-source, hgl-rust-ir and
hgl-rust-layouts. Budget 700 source lines. No runtime type dispatch.

Public surface: `markers(&Plan) -> String`, `construct(&Value, emit) -> String`,
`shape_marker(&Ty) -> String`, `publish(&Ty, payload) -> String` and
`observe(&Ty, input) -> String`. Sparse owning values preserve omissions and
removals recursively; exact originating shapes determine generated marker names.
Written constructor payload order is retained before assembling storage order.
Observation aliases keep prepared input tokens until an owning boundary.
Application emits only supplied children; existing map children merge recursively.
New child allocation uses a statically chosen factory, while existing allocator
metadata validation/reuse remains construction work. Empty recursive application
is rejected before writes. No source-level delta inspection is introduced.

Acceptance follows ordinary-delta-types.md and cases_ordinary_delta_types.md at
spec60a2d7e: nested sparse ownership, removals, equal primitive publications,
source order, future generator retention, typed global exact identity.
Mutants: fill omitted children from held values; discard removals; evaluate
fields in declaration order; retain observation at alias declaration; replace
existing map subtree rather than merge.

`equivalent(ty,left,right) -> String` emits post-run structural comparison.
Generated equivalent methods ignore constructor ordering for sets and keyed
sparse entries, compare exact presence and recurse on children; they never fill
omissions from held state. No general HGL equality operator is admitted.

`structural(&Ty) -> bool` identifies structural endpoint emission at compilation.
The generated delta marker provides allocate, validate, observe, apply and
post-run equivalent methods. Sparse payload storage is opaque to HGL; generated
Rust projection code is the only consumer of its internal list fields.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

Typed set/map key operations are delegated to hgl-rust-keyed. Their ordinary
sparse storage retains exact K; only prepared membership calls use internal IDs.

Prepared finite execution delegates statically typed slot publication, sparse
observation, and direct pass-through emission to `hgl-rust-observed`. Existing
ordinary owning delta construction and post-run comparison remain available.
