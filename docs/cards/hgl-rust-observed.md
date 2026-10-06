# hgl-rust-observed

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
