# Card: hgl-value-constant

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_constant` module of `hgl-semantics` (`crates/hgl-semantics/src/value_constant.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Pure classification of checked ordinary expressions eligible for constant
checking. Uses hgl-rust-ir; initial budget 100 source lines. Public
`context_free(&Value) -> bool` recognizes evaluator-supported closed expressions
and lexical ValueCall argument/local scopes. It rejects external locals,
contextual recipes, prepared configuration, native/capability/global/endpoint
effects and unsupported or unbounded control flow without evaluating anything.
The evaluator retains short-circuit and branch execution semantics.

Acceptance: constant bounds diagnostics, deferred mixed contextual expressions,
external setup locals, and skipped invalid branches in hgl-program regressions.

Delta traversal includes typed Add/Remove keys and both Keyed map operands;
retained keys obey the same constant/provenance checks as child expressions.

`retain_closed` retains checked closed owning constructor subtrees as immutable
configuration recipes, using the caller's static ownership predicate. It does not
evaluate expressions or hooks. Literal family coercions retain their concrete
payload; dynamic constructor children remain at their source positions.
