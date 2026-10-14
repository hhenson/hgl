# Card: hgl-program-keyed

The internal `resolve::keyed` module checks the Spec's independent outputless
dynamic composition subset. It reads HGL source syntax and checked IR only.
`Checker::graph_keyed(module, statement, env)` admits map `values`/`items` and
unbounded-list `elements`/`items`, retaining the exact member endpoint schema.
The generated owner stores the collection connection and every actual external
producer dependency, including REF carriers and capture error producers.
The member placeholder denotes a boundary source, never a payload copy.

Enclosing binding assignments, returned loop results, loop exits and graph-phase
predicates remain outside the Spec's initial independent-body subset. Ordinary
captures retain the Spec's unresolved binding contract and are diagnosed when
used. Key/index expression binding is a Rust implementation gap; unused item
bindings are accepted. Local bindings inside a child remain lexical and cannot
escape the iteration. Nested child ownership retains all actual endpoint edges.

Acceptance includes compile-time source controls for independent bodies, exact
temporal captures and rejected enclosing writes, plus executed per-key capture
controls for state retention, sibling continuation, removal and fresh recreation.
