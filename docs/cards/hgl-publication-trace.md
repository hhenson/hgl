# hgl-publication-trace

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `publication_trace` module of `hgl-semantics` (`crates/hgl-semantics/src/publication_trace.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold validation of a closed publication sequence before graph start.

`validate(&Ty, &[Option<Value>]) -> Result<(), (usize, String)>` starts with empty
membership and reports the first invalid input slot. It checks canonical set
membership, exact complete map keys, sparse child updates, and contiguous growing
list appends and tail removals. Removing a parent discards its descendant state.
Silent slots make no change; empty structural publications remain invalid.

Uses hgl-delta-check for materialized duplicate checks, hgl-composite-keys for
complete identities, and hgl-growing-range for dense index rules. The evaluator
reexports this function.
