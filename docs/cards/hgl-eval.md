# Card: hgl-eval

Ordinary execution and traversal of checked IR form a lower layer depending only on
hgl-value-model and hgl-types. It cannot parse syntax, resolve declarations or
reenter semantic checking. Contextual temporal materialization uses the existing
explicit callback; no execution API or language admission changes.

The ir facade reexports syntax-neutral definitions from hgl-value-model.
Public modules are ir, ir_visit, harness_ir, value_eval, value_operations,
any_values, collection_values, composite_keys, scalar_keys, family_values,
value_constant and publication_trace. Their existing contracts and module
ceilings remain unchanged; aggregate ceiling 2000. publication_trace::materialized
checks closed delta key overlap/duplication; delta expression admission stays in
semantic delta_check.

Semantic facade paths preserve existing clients. Boxing and scalar constructor
admission remain in semantics; the lower evaluator has no Expr, Cursor or Library
dependency. Existing semantic tests exercise the facades and unchanged execution.
DEC0004 reason 3 applies: checked execution and source semantics each exceed 1000
lines, with the dependency boundary enforced by Cargo.
