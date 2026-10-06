# Card: hgl-rust-type-data

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `type_data` module of `hgl-rust` (`crates/hgl-rust/src/type_data.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Serialize exact checked source type metadata through public ty(&Ty)->String.
Uses hgl-source and hgl-rust-enums; budget 120 source lines. Retain nominal
arguments, optional positions, recursive batches, declared enum identity and
publication origins. No expression evaluation or type inference occurs here.
hgl-rust-checked-data re-exports this function for existing emission callers.
