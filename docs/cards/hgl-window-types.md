# Card: hgl-window-types

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `window_types` module of `hgl-types` (`crates/hgl-types/src/window_types.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Canonical rolling size identity shared by source types and cold runtime schemas.
No dependencies or unsafe code; budget 100 source lines. Contract: normative
rolling-publications a3f5cf8.

`WindowKind::{Ticks,Duration}` keeps arrival counts distinct from microseconds.
`Window::new(kind,max,min)` validates positive maximum, positive tick minimum,
nonnegative duration minimum and minimum <= maximum. Immutable Window exposes
kind/maximum/minimum, canonical source_name, and parse for already normalized
integer or microsecond bounds. Source expression evaluation belongs to
hgl-type-sizes; equivalent duration spellings normalize before identity.

Acceptance: rolling compiler tests cover omitted minimum, equivalent duration
spellings, exact kind matching and rejected size/payload boundaries.
