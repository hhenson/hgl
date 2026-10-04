# Card: hgl-body-check

Classify function phases and parse function-level service headers. Uses
hgl-source and hgl-library; budget 220 source lines, no runtime dependency.

Public surface: `runtime_body`, `ordinary_body`, `generator_body`.
Runtime classification follows node-only statement syntax before checking nested placement;
identifier spellings inside expressions, labels and bindings are not phase evidence.
This classifier does not grant identifier admission; reserved-word legality belongs
to source parsing.
const value bodies do not change phase when injecting a service. While and timed
yield are rejected in const value bodies. Body parsing returns declared service
names and lexical statements. Headers admit newline-separated comma lists with
an optional trailing comma, and reject explicit duplicate services.
Generator admission forbids state/cache/lifecycle hooks/for and permits only
clock/logger services in this executable profile. Caller checking resolves
capability bindings, available execution phase, operand types and lexical scope.

Acceptance: pinned generator and logging-value-helper examples, runtime/const
phase diagnostics, lexical parser tests and literal-only yield text regressions.

`composition_body(&mut Cursor) -> Result<Vec<Stmt>, String>` parses a block or
concise return body and rejects trailing syntax. It does not select branches,
resolve locals or evaluate expressions.
