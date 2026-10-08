# Card: hgl-tuple-flow

Own source runtime provenance for ordinary Tuple constant-context checks.
The internal tuple_flow module has an initial 60 source-line budget within the
unchanged hgl-semantics limit. It uses name_check Scope and tuple_phase dependence.

Public surface: defaultable/clonable/debug `Facts`, `Facts::new(BTreeSet<String>)`,
`bind(&str,&Expr,bool)`, `assign(&Expr,&Expr)`, `iteration(&str)`, `merge(&Facts)`, and `nested(&mut Scope, closure)`.
`runtime` exposes known runtime source names. Facts retains private lexical
binding identities, so an inner same-spelling let/var cannot change an outer
binding's provenance. State/cache bindings remain runtime; ordinary assignment
records RHS dependence and can clear it after an unconditional fixed assignment.
Branch/loop checks merge runtime assignments only for matching outer identities.
No source type/category, runtime execution state or provider behavior changes.

Acceptance: unused outer assignments across branches/loops reject constant Tuple
arguments; inner local shadowing preserves an unrelated outer constant member.
Direct aliases, state/cache, injected clock/out/global_state payload dependence
and explicit constant defaults retain ordinary uncatalogued phase diagnostics.

Admitted temporal items/elements loops carry runtime scalar/child bindings.
Iteration shadows an outer lexical identity; merge preserves runtime writes to
other outer variables and cannot transfer the loop binding into its namesake.
Ordinary composition/value-function for loops retain their existing unsupported
profile; no ordinary constant-loop phase behavior is defined by this module.
