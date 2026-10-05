# Card: hgl-rust-prepared-values

Typed prepared-copy emission for nominal ordinary layouts. Depends on hgl-source;
250 source lines. Public `structure(name, fields, marker)` and `enumeration(name)`
and `structure_fields(name, markers)` return PreparedValue implementations for existing GlobalValue markers. The caller
supplies exact child markers; `structure_fields` accepts field-presence markers
selected by the concrete struct emitter without changing the source field type; no source type or schema is inspected during copies.

Bounds merge only during cold preparation. Whole-value checks visit every child
before an infallible copy writes any destination. Independent destination slots
preserve nominal identity and ownership across publication and capture. Enum
markers reuse i64 storage operations without losing their checked nominal type.

Acceptance is the generated finite owning-publication allocation and retention
suite in hgl-program, including strings, provider-backed scalars, nominal structs,
fixed collections, atomic values and structural map removal/reinsertion.
