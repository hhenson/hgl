# hgl-rust-observed

Statically typed publication transport for prepared finite evaluations. `methods` emits exact structural `apply_slot`, `observe_slot`, and `pass` implementations; `apply`, `capture`, and `pass` render leaf or recursive calls. Ordinary values remain in independently owned destination storage. Key visitation borrows cold retained identities. Sparse membership and validity use existing endpoint semantics, and capacity overflow fails rather than allocating.
