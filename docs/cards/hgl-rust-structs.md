# Card: hgl-rust-structs

Status: accepted.

Generate ordinary concrete struct ownership and field-presence operations.
Depends on hgl-source and hgl-rust-prepared-values; budget 230 source lines. Type collection and spelling
remain in hgl-rust-layouts.

Public `marker(&Ty, fn(&Ty)->String, fn(&Ty)->String) -> String` receives a
checked concrete struct plus marker-name and schema emitters. Emit its complete
GlobalValue and PreparedValue implementations. Required fields use their existing markers;
optional positions use hgl_store::Optional<T>, preserving present payload type
and exact nominal identity. Declaration-ordered positions remain stable.

Retain, prepare, install, read, commit and release delegate to each typed field
marker. No runtime schema search, source type coercion or optional field access
is introduced. Optional values use native Option only as their internal owning
field representation. Shared optional fixture execution verifies the emitted
representations and capture conversions in both build profiles.
