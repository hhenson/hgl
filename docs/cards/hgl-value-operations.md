# Card: hgl-value-operations

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_operations` module of `hgl-semantics` (`crates/hgl-semantics/src/value_operations.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Ordinary checked scalar unary and binary operators and phase-independent errors,
extracted from hgl-value-eval. Uses hgl-source and hgl-rust-ir; budget 250 source
lines. Exposes unary, binary and EvalError::{Operation,ContextRequired,Unsupported}
with the existing Display/Error behavior. Preserves exact scalar comparison,
checked datetime/duration arithmetic and existing integer-overflow diagnostics.
No parsing, provider execution, scheduling or lexical scope ownership.

EvalError implements From<String> as an operation failure. Exact ordinary set/map ==/!= delegates recursive unordered comparison to hgl-collection-values.

bytes(source arguments, typed checker)->Result<Value,Issue> validates constructor arity/type and supplies list<i64> context for empty lists. Evaluation produces constructed bytes after ordered argument evaluation; EvalError::Coded carries value.byte_range. Exact byte equality/order follows unsigned contents. May use endpoint_check.

Explicit constructor checking delegates to hgl-value-constructors, preserving the existing constructor/bytes public paths. Any binary execution delegates to hgl-any-values. BOOL-1 supplies false-before-true order and exact Boolean equality/hash.

Complete atomic payload conversion preserves signal-only input authority.

Checked execution lives in hgl-eval behind the existing hgl-semantics facade.
Source constructor admission remains above that dependency boundary.
