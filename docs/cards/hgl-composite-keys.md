# Card: hgl-composite-keys

Cold exact identity for complete scalar, positional tuple and concrete struct
collection keys. Depends on source, rust-ir and scalar-keys; budget 150 lines.
No runtime lookup, hash-only identity or implicit conversion.

Key derives Clone/Debug/Eq/Ord and has Scalar(scalar_keys::Key) and
Fields(Ty, Vec<Option<Key>>) variants. scalar(Literal) forms index/scalar identity;
known(Value) returns an optional complete identity, deferring provider leaves;
key(Value) requires all leaves materialized. Field order is canonical, with exact
nominal types, tuple positions and every optional presence bit retained. Required
missing fields, duplicate/out-of-range positions and mismatched child types fail.
Signed zero and NaN rules come from scalar-keys. Recursive/family/collection/ref
components remain unsupported. Callers retain written evaluation order separately.
