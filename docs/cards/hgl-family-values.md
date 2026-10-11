# Card: hgl-family-values

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `family_values` module of `hgl-semantics` (`crates/hgl-semantics/src/family_values.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Checked ordinary family widening and canonical closed payload ownership. Depends
on source and rust-ir; budget 100 source lines. No runtime discovery or projection.

coerce(expected,value) validates exact concrete membership or explicit ancestor
identity and emits internal Unary("family", operand). retain(target,value) handles
that checked cold conversion once, flattening a subfamily wrapper while preserving
the exact concrete member type and independently retained fields. Canonical family
values are Construct([(canonical member index, concrete Value)]). concrete(value)
borrows that payload for exact-tag-first equality and harness comparison.

equal(a,b) reexports authoritative fallible ordinary equal_data. Exact concrete tags precede recursive provider/box operations; failures retain their ordinary error identifiers. The existing nominal identity controls require exact Ok(true/false) results.
