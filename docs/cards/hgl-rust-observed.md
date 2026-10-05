# hgl-rust-observed

Statically typed publication transport for prepared finite evaluations. `methods` emits exact structural `apply_slot`, `observe_slot`, and `pass` implementations; `apply`, `capture`, and `pass` render leaf or recursive calls. Ordinary values remain in independently owned destination storage. Key visitation borrows cold retained identities. Sparse membership and validity use existing endpoint semantics, and capacity overflow fails rather than allocating.

`text(&Value)` emits direct reserved text composition for pure input/literal
concatenation, validating byte length and inputs before one publication.

Growing typed slot application and pass-through validate canonical ranges against destination length before mutation. Capture retains changed child publications and the complete removed tail, including shrink to a valid empty root.
