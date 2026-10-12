# Card: hgl-eval

Checked ordinary execution forms a lower layer whose only dependencies are
hgl-value-model and hgl-types. It cannot parse source syntax, discover providers,
resolve declarations/overloads, or reenter semantic checking. Native operations
are invoked only through the caller's explicit checked execution context.

Model IR is reexported as `ir`. Public modules are `ir`, `ir_visit`, `harness_ir`, `value_eval`, `value_operations`,
`any_values`, `collection_values`, `composite_keys`, `scalar_keys`, `family_values`,
`value_constant`, `publication_trace` and `key_context`. Their existing public contracts and
module ceilings remain unchanged. `publication_trace::materialized` checks closed
delta key overlap/duplication; expression admission remains in semantic
`delta_check`. The aggregate ceiling is 2000 lines.

Semantic facade paths reexport checked IR and pure operations. Boxing and scalar
constructor syntax admission stay in semantics; the lower evaluator has no Expr,
Cursor or Library dependency. Existing semantic tests continue validating both
facades and execution, including required-default phase boundaries and owning Any.

The layer implements DEC0004 reason 3: checked execution and source semantics each
exceed 1000 lines, and Cargo enforces their one-way dependency.
