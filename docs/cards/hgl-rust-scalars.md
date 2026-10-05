# Card: hgl-rust-scalars

Status: accepted

Scalar source-to-Rust spelling helpers extracted unchanged from hgl-rust-layouts.
Uses hgl-source and hgl-types; budget 150 source lines. Preserve and re-export
existing scalar helper signatures at their original public paths; list any new
signature here before adding it. Extend the mappings for CivilDateTime, ZoneId,
ZonedDateTime and ZonedTime with exact scalar identity. No runtime/provider work lives here.
