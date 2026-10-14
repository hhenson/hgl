# Card: hgl-rust-capture-graphs

`hgl-rust-codegen::capture_graphs` emits the Spec compiled graph capture boundary.
`child(plan,capture,members,graph,inputs)` emits exact external input boundaries
with no public child output. `node(plan,id,shape)` emits a typed passive bundle,
private Error output, and one reusable prepared child. `support()` emits cold
boundary adaptation and generation-aware live refresh. `preparation(plan,
recursive_prepare,normal_targets)` emits recursive child preparation, private
error storage, and cold bundle designation assembly. The normal-target callback
uses checked reference provenance and supplies exact endpoint paths.

The normal field retains its original endpoint, including a callable that
forwards an external member without adding executable child nodes. A capture evaluation failure
writes only the private canonical Error endpoint. The error persists until the
owning scope ends; a successful later pass does not clear it. Start and stop
failures escape. Boundary refresh preserves live child state and skips unchanged
source bindings. Nested discarded passes preserve future deadlines.

Possible normal endpoint designations are interned into complete bundle
references before startup. A changing REF carrier selects an existing reference
by identity during evaluation, without copying normal values, interning, or
payload type dispatch. This finite prepared profile requires exact normal target
provenance and bounded child preparation. Generic canonical Error payload costs
are not claimed to satisfy a universal allocation bound.
