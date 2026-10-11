# Card: hgl-ir-visit

Model `ir_visit` owns syntax-neutral checked IR traversal with explicit subtree
pruning and may use Model `ir`. Eval `ir_visit` reexports those walkers and owns
harness traversal, using `ir` and `harness_ir`. Optional reporting targets are 260 lines for Model and 60 for Eval;
module dependency boundaries remain enforced.

Public surface: value, statements, hooks, value_mut, statements_mut, nodes and nodes_mut and harness. Traversal visits source-order operands and retained ordinary helper bodies; false stops only the selected expression subtree. nodes and nodes_mut rewrites lexical statements while preserving independent helper-local scopes. harness includes setup/assertions, every supplied eval argument and sequence, expected capture values and nested raises/if bodies.

This policy never executes IR or changes source admission. Backend storage/layout collectors and ownership proofs share it so newly admitted kinds cannot be omitted from separate handwritten walks.

Acceptance: all existing IR tests and hidden Any payload layout collection, including silent inputs, cold captures, nested selectors and helper bodies.

`hooks_mut` rewrites the same start/stop/generator/handler operand boundaries as `hooks`, sharing ownership metadata preparation across backend passes.

Read-only lexical nodes and mutable nodes preserve helper scope boundaries for representation-use analysis and immutable literal normalization. Delta and key discovery reuse expression traversal while retaining explicit helper/capture pruning.

Checked execution and harness traversal live in hgl-eval behind the existing
hgl-semantics facade. Model has no harness or evaluator dependency.
Source constructor admission remains above that dependency boundary.
