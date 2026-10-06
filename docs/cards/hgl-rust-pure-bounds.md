# hgl-rust-pure-bounds

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `pure_bounds` module of `hgl-rust` (`crates/hgl-rust/src/pure_bounds.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold analysis of checked pure text expressions. `text_factor` follows ordinary
local construction, field access and assignment to bound concatenation byte
width relative to retained source widths. It invokes no hooks, ordinary helpers,
native providers or constructors; its result is a storage bound, never a value.

Multipliers compose along actual node dependencies; independent branches combine
by maximum. The whole-adapter proof excludes unknown owning growth before these
bounds are used for prepared execution. The multiplier alone is not a proof for
mutable loop bodies or opaque owning-return functions.
