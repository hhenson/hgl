# Card: hgl-rust-value-convert

Exact cold native conversion of retained checked ordinary values. Uses source,
checked-data emission, layouts, families and collections; budget 320 lines.

`decode(&Ty,&str)->String` emits checked-value to independently owning native
conversion. `encode(&Ty,&str)->String` emits exact native capture metadata.
Types, optional presence, concrete family tags, recursive members and sparse
delta origins survive both directions. Complete sets/maps retain exact schema,
using list-backed members or typed key/value tuples without temporal patching.

Acceptance: existing prepared captures and shared atomic container debug/release
runs; conversion occurs outside hot evaluation and never replays provider recipes.
