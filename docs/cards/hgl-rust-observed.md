# hgl-rust-observed

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `observed` module of `hgl-rust` (`crates/hgl-rust/src/observed.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Statically typed publication transport for prepared finite evaluations. `methods` emits exact structural `apply_slot`, `observe_slot`, and `pass` implementations; `apply`, `capture`, and `pass` render leaf or recursive calls. Ordinary values remain in independently owned destination storage. Key visitation borrows cold retained identities. Sparse membership and validity use existing endpoint semantics, and capacity overflow fails rather than allocating.

Growing typed slot application and pass-through validate canonical ranges against destination length before mutation. Capture retains changed child publications and the complete removed tail, including shrink to a valid empty root.
Composite key application resolves component IDs directly from prepared fields.
Capture copies retained scalar leaves and optional presence into reserved typed
slots, without materializing an owning composite intermediary. Generated exact
key methods own domain selection; observation performs no type/name lookup.
`text(value,result)` receives the declared temporal result. Scalar String results
use prepared scalar text; rolling<str,...> uses prepared rolling_text so valid
metadata, retained arrival, count/span and readiness advance together. Other
result kinds do not select this optimization. Pure source fragments are measured
before any destination mutation and copied directly into prepared storage.

read emits exact scalar, rolling, atomic and structural payload reads for scoped iteration children, sharing ordinary publication transport rather than assuming every child is rolling.

input renders scalar or shaped input binding names for queries, including scoped iteration inputs; invalid non-input IR is rejected by its checked invariant.

Empty sparse slot application and pass-through establish invalid destination
validity. Repeated valid empties are silent. Observation retains present empty
child entries separately from omission, and configured deltas reuse their
prepared slots rather than cloning owning sparse storage during evaluation.
