# Card: hgl-family-values

Checked ordinary family widening and canonical closed payload ownership. Depends
on source and rust-ir; budget 100 source lines. No runtime discovery or projection.

coerce(expected,value) validates exact concrete membership or explicit ancestor
identity and emits internal Unary("family", operand). retain(target,value) handles
that checked cold conversion once, flattening a subfamily wrapper while preserving
the exact concrete member type and independently retained fields. Canonical family
values are Construct([(canonical member index, concrete Value)]). concrete(value)
borrows that payload for exact-tag-first equality and harness comparison.

equal(a,b) compares exact concrete tags and then recursively compares retained
ordinary fields/lists. This is cold ordinary equality; runtime native family
representations include the discriminator in their ordinary typed equality.
