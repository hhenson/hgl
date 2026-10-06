# Card: hgl-rust-structs

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `structs` module of `hgl-rust` (`crates/hgl-rust/src/structs.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

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

Finite recursive members emit named owning tuple structs and RecursiveTarget
identities. Optional internal edges select Optional<Recursive<T>> markers;
ordinary nonrecursive fields retain their existing storage markers. Typed owning
retention recurses through finite present values; prepared bounds and copies use
the same declared field positions without schema lookup.

Generated recursive markers include typed ordinary_equal methods. Comparison visits complete finite present fields and delegates nested collection equality to hgl-rust-collections, ignoring collection entry order without weakening nominal identity or optional presence.
