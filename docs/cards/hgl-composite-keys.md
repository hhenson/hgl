# Card: hgl-composite-keys

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `composite_keys` module of `hgl-eval` (`crates/hgl-eval/src/composite_keys.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

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

Native leaves remain deferred in source-known identities. Cold key_context uses
the same recursive formation with fallible provider identities. Field shape
metadata comes from the authoritative ordinary-value shape policy. The module
belongs to hgl-eval and may also use value_eval for its shared error contract.
