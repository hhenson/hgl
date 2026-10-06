# Card: hgl-window-types

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
