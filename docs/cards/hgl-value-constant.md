# Card: hgl-value-constant

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_constant` module of `hgl-eval` (`crates/hgl-eval/src/value_constant.rs`), reexported through `hgl-semantics`; `cargo xtask ci` reports its size and enforces its "may use" list.

Pure classification of checked ordinary expressions eligible for constant
checking. Uses hgl-rust-ir and the shared ir_visit traversal; initial budget 100 source lines. Value::closed lives here alongside context_free and recognizes only independently owning literal/void/list/constructor/captured/delta forms whose descendants are also closed. Native/provider recipes remain deferred. Public
`context_free(&Value) -> bool` recognizes evaluator-supported closed expressions
and lexical ValueCall argument/local scopes. It rejects external locals,
contextual recipes, prepared configuration, native/capability/global/endpoint
effects and unsupported or unbounded control flow without evaluating anything.
The evaluator retains short-circuit and branch execution semantics.

Acceptance: constant bounds diagnostics, deferred mixed contextual expressions,
external setup locals, and skipped invalid branches in hgl-program regressions.

context_free also shares ir_visit traversal while pruning ordinary calls into
their own lexical argument/local scope. Its admitted forms remain unchanged;
provider calls are rejected as required constants without execution.

Delta traversal includes typed Add/Remove keys and both Keyed map operands;
retained keys obey the same constant/provenance checks as child expressions.

`retain_closed` retains independently closed owning constructor subtrees as
immutable configuration recipes, preserving nested family owners. It classifies
IR without executing provider or constructor bodies. Native recipes remain
deferred and preserve their checked invocation phase.
