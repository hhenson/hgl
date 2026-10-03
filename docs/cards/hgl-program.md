# Card: hgl-program

Check closed HGL graphs and tests against source libraries; delegate checked
plans to `hgl-rust` for Rust emission. Uses `hgl-source`, `hgl-library`,
`hgl-documentation`, `hgl-rust`. Budget: 2200 source lines. No third-party
dependencies. Source linking, type/phase/proof checks and eval wiring stay here;
checked backend IR and Rust generation belong to `hgl-rust`.

Public surface: `compile`, `compile_files`, opaque `Program`, `emit_rust`;
`compile_tests`, `compile_tests_files`, opaque `Suite`, `emit_tests`. File loaders
include explicit parts and recursively load libraries, excluding test/example
directories. Embedded tests are indexed; production emission excludes them.

The supported slice includes scalar streams (bool, i64, f64, str, date, time,
datetime, duration), imports, generic operator selection with native scalar
requirements and finite type-domain constraints, defaults, contextual result inference, graph composition, const
lifts, ordered guarded handlers, state/cache, scalar input activity, source
alarms, reference capture/following, and bool/i64 set membership. Set bodies
mutate `out`; `elements(input, added)` supplies typed elements. State resets
for each fresh graph; checkpoint recovery is outside this slice.

`eval` resolves ordinary `hgraph.std::replay` and `hgraph.std::record` operator
contracts and checks their selected HGL bodies through the normal resolver.
The supplied library must contain those declarations, implementations and
instantiations; `compile_tests` does not inject embedded operator definitions.
Generated executables depend on `hgl-std-native` for typed buffer primitives.
Dense
sequences use one microsecond per cell. Silent/empty sequences take their type
from concrete parameters; integer literals in f64 slots are converted before
emission. Expected values are checked against the result type. Each evaluation
gets a fresh graph. Length comes from input cells and actual output ticks,
never the expected sequence. A mismatch or node error fails the executable.

Test helpers have a module-wide scope; production calls cannot see them.
The current test body accepts direct `assert eval(...) == [...]` and outputless
`eval(...)` statements. General bool assertions, harness locals, timed input,
structural delta literals and empty generic sequences remain unsupported.
Unused library bodies are not advertised as implemented: reachable unsupported
forms produce diagnostics. This is not the full language checker.

Native strings cross the Rust value interface as `&str`; returned text is owned.
HGL owns guards, scheduling, state and formatting composition. Rust implements
only the selected native scalar signatures. Selected source docs remain in
emitted comments. No operator name lookup occurs on ticks.

Acceptance: the pinned standard library's 83 tests/132 evaluations,
plus empty/silent/equal ticks, delayed output, fresh state, helper isolation,
wrong values/lengths and propagated node errors. `cargo xtask ci` runs these in
debug and release. The original const/debug graph regressions remain.

Node-scoped `replay_input` and `capture` are non-value capabilities. The first
requires a scalar source; the second an outputless sink with one scalar input.
They cannot escape or appear in value/composition bodies. The checker enforces
ADR0016's exact operation names, positional/named arguments, result types and
start/evaluation phases. Start hooks use ordinary checked statements, native
calls, conditions and scalar cache access. Clock reads and source alarm calls
are lowered through the existing context; temporal input/output access and
return publication are rejected in start.

Each eval plan binds replay literals to one checked source and its capture to
one checked sink. Generated constructors own fresh typed storage per graph
instance. The graph instance is the run identity; no externally supplied buffer
or identity can cross that boundary. Typed fields enforce role/payload, a unique
capture field enforces one writer, and provider construction validates input
length/time. Unconfigured capability nodes fail construction before any start.
Binding does not call begin: only the record operator's HGL start hook begins capture.
After stop, generated code transfers owned capture ticks, drops the graph and
passes the ticks to testkit observation/comparison. It adds no runtime recorder.

`delta_value(input)` checks the concrete instance of the endpoint-derived delta
relationship. The admitted eight scalars have delta type equal to scalar type;
structural delta lowering is not yet implemented and those instances are diagnosed.
Formal `signal` parameters retain their signal identity even when the producer
has a scalar payload; they are excluded from both delta access and capture binding.
Only runtime evaluation can read delta metadata. Endpoint identity and proof of
both valid and modified are required; copied payloads/consts are rejected.
Handler and local short-circuit/conditional facts establish those guarantees.
`delta(input)` is not an accessor intrinsic; `delta<T>(...)` is a constructor.

Acceptance also includes mutated source operator bodies proving execution of the
selected HGL handlers, missing-binding construction failure before start, native
start failure, capability operation/phase/type/escape errors, translated buffer
errors through generated nodes, generic delta forwarding and endpoint-specific
proof checks. No runtime control flow is selected by replay/record operator name.

Implicit handler-selector normalization is applied to the scalar-input profile.
Existing structural guard emission is retained: applying scalar normalization to
reference startup handlers exposes an unresolved mismatch between reference
binding modification time and startup activation. Structural delta metadata and
that normalization/runtime integration remain outside this completed profile.

Capability actions and non-clock queries use receiver-first prelude calls
with a direct injected name as the first positional argument; remaining arguments use ordinary
positional/named binding. Dotted capability methods are not aliases. Replay
uses `len(replay_input)` and evaluation-only `replay_input[index]`. Its contextual
nullable result can enter immutable inferred locals but cannot escape into
mutable locals, state/cache, ordinary calls or output without presence proof.
Null comparisons refine the particular local on both branches, through negation
and short-circuit evaluation. Continuing paths intersect their guarantees;
a terminating branch contributes no continuing path. Unrefined copies require
their own guard. Bare runtime return terminates without publishing; `return null`
is not a no-output operation. Scalar endpoint `delta_value` proofs remain separate.

Clock observations use read-only properties of the direct injected clock:
`clock.evaluation_time` and `clock.next_cycle_evaluation_time` produce owned
`datetime` values in supported start/evaluation/stop hooks. A local snapshots the
read value. Property invocation, free-function clock aliases, property writes,
unknown properties and non-capability receivers are rejected. `clock.now`
requires wall-clock support absent from this backend and is diagnosed explicitly.
Scheduling remains limited to start/evaluation hooks; capability access from
ordinary value helpers remains outside this profile.

Scalar `global_state` is an ordinary run-wide keyed facility, admitted in
start/evaluation/stop without temporal shape or source/sink role constraints.
Get uses an ordinary concrete scalar expected type, never a key spelling or
an enclosing temporal shape; unconstrained reads are diagnosed. Annotated
locals resolve selected generic bindings. Set takes ordinary scalar payloads;
capabilities, endpoint references, signals, aggregates and nullable values do
not enter this scalar profile. Stop bodies use checked ordinary statements;
input/output publication and evaluation-only operations remain unavailable.
No new replay or recording representation follows from this facility.

Global-state keys currently admit string literals and resolved const string
parameters. General const expressions (including literal concatenation) and
hook-local keys are not evaluated as keys by this backend; diagnostics identify
that subset limitation. Temporal keys are unsupported by the source profile. Equal
keys have one exact scalar type throughout the statically assembled plan; known
conflicts are checking errors. Per-node typed entry requirements are emitted for
construction preflight; binding does not initialize an entry. Direct annotated
initializers, returns, assignments and conditions supply get expected types;
nested expression and overload-argument inference are outside this subset.

Initialized runtime `var` bindings of the eight primitive scalar types are
writable owned locals, including in lifecycle hooks and lifted ordinary value
function bodies. `let`, parameters and `for` bindings remain read-only. Branch
scopes preserve each binding identity across shadowing. Scalar assignments
retain their exact type; local `+=` uses existing addition typing (i64, f64,
str), while cache increments retain their i64 profile. Primitive global get
initializes an owned local, so local mutation never implicitly writes the entry.
Uninitialized locals, aggregate borrows and general value-helper runtime calls
remain outside this backend subset. No value-type qualifier is introduced.

Runtime hook locals admit nongeneric ordinary structs with required primitive
or nested struct fields. Constructors require every field once by name and
check exact nominal types. Owning local initialization, constructor retention,
and assignment copy independently (value-mutability, VAL-17). `var` admits
whole-value and nested field replacement; `let` is recursively read-only.
Primitive field `+=` uses existing addition typing. Field projections retain
the root's write authority. Unsupported optional/default/generic/recursive
schemas and aggregate global entries are diagnosed; no list operations or
aggregate temporal ports/helper parameters follow from this local slice.

Acceptance: source fixtures executed as emitted Rust through lifecycle hooks;
nested text/value copy independence, mutable field/whole replacement, branch
shadowing, rejected readonly writes, missing/duplicate/wrong-type fields and
nominal mismatch. Existing scalar source and global-state tests remain green.

Imported structs preserve qualified identity and declared layouts. Local names
precede imports; cross-module access requires export, and selected exported
layouts reject unexported reachable struct fields (ADR 0013). Declaration
checks remain lazy in this executable subset: unused schemas are not certified.
All eight primitive field types are covered by source eval assertions.

The checked constructor preserves supplied source order and declared-field
indices after validating the entire call (`struct-constructor-order.md`).
Backend assembly must not reorder or reexecute its argument expressions.
