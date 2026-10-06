# Card: hgl-scalar-keys

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `scalar_keys` module of `hgl-semantics` (`crates/hgl-semantics/src/scalar_keys.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold ordinary scalar collection identity, specified by scalar-collection-keys
(spec 06e576a). Depends only on hgl-source; budget 100 source lines.

Public surface: opaque ordered Key and key(&Literal) -> Result<Key,String>.
Preserve exact ordinary type, enum declaration identity, exact zone spelling,
wall time, instant and offset as applicable. Normalize f64 signed zero; retain
both infinities distinctly; explicitly reject NaN. Ordering is an internal cold
index implementation, never a source ordering operation. No provider lookup,
stringification, numeric coercion or per-tick storage belongs here.

Acceptance: all scalar identities, signed zero, infinities, NaN, cross-type and
cross-enum differences, exact zone aliases and complete zoned identity.
