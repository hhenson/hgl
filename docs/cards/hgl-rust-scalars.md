# Card: hgl-rust-scalars

Status: accepted

Scalar source-to-Rust spelling helpers extracted unchanged from hgl-rust-layouts.
Uses hgl-source and hgl-types; budget 150 source lines. Preserve and re-export
existing scalar helper signatures at their original public paths; list any new
signature here before adding it. Extend the mappings for CivilDateTime, ZoneId,
ZonedDateTime and ZonedTime with exact scalar identity. No runtime/provider work lives here.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.
