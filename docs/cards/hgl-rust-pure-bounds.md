# hgl-rust-pure-bounds

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `pure_bounds` module of `hgl-rust` (`crates/hgl-rust/src/pure_bounds.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Cold analysis of checked pure text expressions. `text_factor` follows ordinary
local construction, field access, transparent unary wrappers and assignment to
bound concatenation byte width relative to retained source widths across start,
stop, handlers and generator bodies. It invokes no hooks, ordinary helpers,
native providers or constructors; its result is a storage bound, never a value.
Checked ordinary call bodies use a fresh parameter-bound environment derived
from their arguments. Nested expressions, guards and internal helper locals
contribute their maximum storage width even when the call returns a non-owning
scalar. Parameter bounds retain constructed field shapes. Call analysis does not
change the separate finite-execution proof or admit recurrent owning growth.

The fixed native formatter envelope is multiplied by checked concatenation
widths before reserving boxes. Formatting still uses the existing native String
path: this capacity guarantee does not claim allocation-free native formatters.
Authored single-input long-value regressions avoid unrelated list frame widths
masking a missing bound, and preserve the separate computed-input text
zero-allocation controls.

Multipliers compose along actual node dependencies; independent branches combine
by maximum. The whole-adapter proof excludes unknown owning growth before these
bounds are used for prepared execution. The multiplier alone is not a proof for
mutable loop bodies or opaque owning-return functions.

byte_limits(type iterator, index function)->String emits cold Bytes width propagation from exact ordinary i64 list capacities. It does not inspect runtime contents or move constructor execution.

`global_growth` keeps empty recorder row bounds separate from configured ordinary global rows plus bounded appends; it emits capacity arithmetic without executing a hook.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.

Capacity setup first includes native scalar envelopes and applies the checked
text multiplier; only then does it derive Any frame widths from those bounds.
This preserves one cold bound for direct text outputs and computed boxed leaves.

Prepared constructor recipes retain their checked origin for text bounds; staging
into an owning slot does not hide concatenation width. The module may use
constructor_owners to recover that origin without evaluating the recipe.
