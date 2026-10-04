# Card: hgl-eval-data

Compile-time preparation for finite eval publication traces. Depends only on
hgl-source and hgl-rust-ir; budget 350 source lines. No runtime dispatch.

Public `validate(shape, slots)` checks a fresh trace's canonical nonempty delta
admission and returns its first rejected position and reason. Sparse children
retain their membership state between positions; removals destroy that child's
validation state. This never fills omissions from held data or changes source
constructor evaluation order. This profile has no invalidation: every admitted
publication makes its target valid. A child entry survives removal of its final
member, preserving valid-empty membership until its parent map removes it. No
separate validity bit is needed for these admission decisions. Type identity and constructor shape are already
checked by the frontend. Excluded empty publications remain explicit errors.

Public `timed(entry_type, slots)` constructs an ordinary unbounded list of exact
TimedValue entries for present dense slots. It preserves absence and the separate
caller-owned dense horizon; absolute times use the specified first eval instant
and minimum interval. All values are independently owned closed compiler data.

Public `sequence(expressions, hint, check)` retains dense absence, checks each
present expression against the exact delta type, and infers a missing target
from the first explicit delta origin or scalar. Its frontend callback performs
source checking and closed ordinary evaluation; empty generic inputs cannot
invent an origin. This helper does not validate endpoint membership.

Ordinary test setup uses `Scope: Default`, backed by hgl-value-eval (an allowed
dependency). `apply(&Stmt, check: impl FnMut(&Stmt, &mut BTreeMap<String, Value>,
&mut usize) -> Result<Statement, String>) -> Result<(), String>` checks and
executes one setup statement once. `values(&mut self) ->
Result<BTreeMap<String, Value>, String>` supplies closed, independently retained
ordinary bindings to each fresh eval/input/expected-value checker. The callback
preserves ordinary let/var permissions, lexical scope and constant-call rules.
`statement(&mut Cursor) -> Result<Stmt, String>` parses supported ordinary setup
bindings, assignments and calls using the existing source AST; unsupported
composition forms remain explicit diagnostics. `recording_key(&Plan) -> String`
returns the existing run-owned recording entry identity without runtime lookup.

`evaluation(Expr, assertion: bool) -> Result<Evaluation, String>` parses the
named target, ordered arguments and optional dense expected sequence without
resolution or evaluation. `Evaluation` has public `function: String`,
`arguments: Vec<(Option<String>, Expr)>` and
`expected: Option<Vec<Option<Expr>>>` fields.
