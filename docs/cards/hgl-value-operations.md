# Card: hgl-value-operations

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_operations` module of `hgl-semantics` (`crates/hgl-semantics/src/value_operations.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Ordinary checked scalar unary and binary operators and phase-independent errors,
extracted from hgl-value-eval. Uses hgl-source and hgl-rust-ir; budget 250 source
lines. Exposes unary, binary and EvalError::{Operation,ContextRequired,Unsupported}
with the existing Display/Error behavior. Preserves exact scalar comparison,
checked datetime/duration arithmetic and existing integer-overflow diagnostics.
No parsing, provider execution, scheduling or lexical scope ownership.

EvalError implements From<String> as an operation failure. Exact ordinary set/map ==/!= delegates recursive unordered comparison to hgl-collection-values.
