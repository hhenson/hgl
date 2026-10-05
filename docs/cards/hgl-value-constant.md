# Card: hgl-value-constant

Pure classification of checked ordinary expressions eligible for constant
checking. Uses hgl-rust-ir; initial budget 100 source lines. Public
`context_free(&Value) -> bool` recognizes evaluator-supported closed expressions
and lexical ValueCall argument/local scopes. It rejects external locals,
contextual recipes, prepared configuration, native/capability/global/endpoint
effects and unsupported or unbounded control flow without evaluating anything.
The evaluator retains short-circuit and branch execution semantics.

Acceptance: constant bounds diagnostics, deferred mixed contextual expressions,
external setup locals, and skipped invalid branches in hgl-program regressions.
