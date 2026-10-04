# Card: hgl-value-eval

Direct execution of checked ordinary value IR during constant evaluation and
graph construction. Depends only on `hgl-source` and `hgl-rust-ir`; budget 500
source lines. No runtime or third-party dependencies.

Public surface: `Evaluator` with `Default`, `value`, `statement` and `bind_failed`;
`constant(&Value) -> bool`;
`EvalError::{Operation(String), Unsupported(String)}` with `Display` and `Error`.
`statement` returns an optional yielded ordinary value. `Operation` identifies
an evaluated operation failure; callers report it at checking for constant
evaluation and construction for wiring. `Unsupported` identifies a backend
gap or invalid checked IR, never a deferred construction failure.

Results are closed `Literal`, `List`, `Construct`, or `Void` IR. Owning local
initialization, assignment, constructor arguments and push retain independent
recursive copies. Function arguments are evaluated in order, then installed
as readonly locals in a fresh invocation. Branch bindings end lexically while
writes to outer variables remain visible. Projection authority follows the
owning root; indexed replacement and fixed-list growth are rejected.

The interpreter supports ordinary expression operators with defined scalar
semantics, local ownership, field assignment, list observation/growth, direct
value calls, conditionals and yield. Native/capability effects, temporal
expressions and temporal iteration are explicitly unsupported. Underspecified
integer overflow is unsupported rather than choosing an implicit policy.

Acceptance: focused executed IR tests for nested independent retention, readonly
arguments, source-order constructors, lexical branches, direct calls, bounds
failure, fixed-list growth rejection and scalar operator evaluation.

`bind_failed` preserves an already-checked unavailable wiring local so checking
can continue after an operation failure. Reading or projecting that typed
`WiringFailure` repeats its operation failure; it is never a successful constant.
The frontend's configuration binder recognizes this internal marker only while
carrying a recorded construction failure.

Runtime while/timed yield and backend-only hoisted generator locals are rejected
as unsupported evaluator IR; this evaluator never executes source generators.

Datetime/duration add/subtract and duration negation use checked microsecond
arithmetic. Overflow is an Operation error (`time arithmetic overflow`) at the
ordinary evaluation phase; the same typed IR fails at runtime before subsequent
expression evaluation. Integer overflow policy remains unchanged.

Closed ordinary Delta constructors evaluate each supplied child in source order
and retain independent IR data. Observation locals are runtime-only and cannot
be folded as ordinary constant data. Empty sparse data formation is valid and
performs no publication or fresh-endpoint trace admission.

## Temporal scalar preparation

Add value_with and statement_with, each receiving a mutable materialization
callback FnMut(&TemporalLiteral) -> Result<Literal, EvalError>. Propagate that
callback through recursive evaluation and ordinary helper calls in source order.
EvalError::ContextRequired distinguishes missing construction context. Existing
value/statement reject contextual recipes with that error; Unsupported remains a
backend defect, never a request to defer. Constructed Literals never invoke the
callback again. The existing source-line budget remains unchanged.

`Evaluator` is cloneable for isolated cold branch checking. Cloning retains
independent ordinary local values and authority; it does not duplicate runtime
handles or execute effects. Only the selected branch's evaluator is retained.
