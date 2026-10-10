# Card: hgl-rust-value-convert

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_convert` module of `hgl-rust` (`crates/hgl-rust/src/value_convert.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Exact cold native conversion of retained checked ordinary values. Uses source,
checked-data emission, layouts, families and collections; budget 320 lines.

`decode(&Ty,&str)->String` emits checked-value to independently owning native
conversion. `encode(&Ty,&str)->String` emits exact native capture metadata.
Types, optional presence, concrete family tags, recursive members and sparse
delta origins survive both directions. Complete sets/maps retain exact schema,
using list-backed members or typed key/value tuples without temporal patching.

Acceptance: existing prepared captures and shared atomic container debug/release
runs; conversion occurs outside hot evaluation and never replays provider recipes.

Bytes encode and decode as exact Literal::Bytes and Vec<u8>; constructor materialization precedes replay capture conversion. No text encoding or implicit conversion exists.
