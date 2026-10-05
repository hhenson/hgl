# Card: hgl-program

Check closed HGL graphs and tests against source libraries; delegate checked
plans to `hgl-rust` for Rust emission. Uses `hgl-source`, `hgl-library`,
`hgl-documentation`, `hgl-rust`, `hgl-value-check`, `hgl-value-types`,
`hgl-value-bind`, `hgl-value-eval`, `hgl-value-access`. Budget: 2200 source lines. No third-party
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
The current test body accepts direct `assert eval(...) == [...]`, outputless
`eval(...)`, deterministic ordinary bool assertions and ordinary test bindings.
Timed input and untyped empty generic sequences remain unsupported. Structural delta literals use
the exact originating-shape publication profile described below.
Unused library bodies are not advertised as implemented: reachable unsupported
forms produce diagnostics. This is not the full language checker.

Native strings cross the Rust value interface as `&str`; returned text is owned.
HGL owns guards, scheduling, state and formatting composition. Rust implements
only the selected native scalar signatures. Selected source docs remain in
emitted comments. No operator name lookup occurs on ticks.

Acceptance: the pinned standard library tests and evaluations,
plus empty/silent/equal ticks, delayed output, fresh state, helper isolation,
wrong values/lengths and propagated node errors. `cargo xtask ci` runs these in
debug and release. The original const/debug graph regressions remain.

Replay and record receive ordinary const values and keys. Their HGL bodies
use generator scheduling and prepared global-state entries. Eval retains dense
input lengths separately from present timed data and obtains an independently
owned recording after stop. Each graph owns its ordinary run storage.
Start hooks use ordinary checked statements, native calls, conditions and cache
access; temporal input reads and return publication remain rejected there.

`delta_value(input)` checks the concrete instance of the endpoint-derived delta
relationship. The admitted eight scalars have delta type equal to scalar type; structural
instances have an exact ordinary Delta type retaining the complete origin.
Formal `signal` parameters retain their signal identity even when the producer
has a scalar payload; they are excluded from delta access and recording.
Only runtime evaluation can read delta metadata. Endpoint identity and proof of
both valid and modified are required; copied payloads/consts are rejected.
Handler and local short-circuit/conditional facts establish those guarantees.
`delta(input)` is not an accessor intrinsic; `delta<T>(...)` is a constructor.

Acceptance also includes changed source operator bodies proving execution of
selected HGL statements, ordinary list bounds/missing-entry errors, native start
failure, removed bespoke injectable rejection, generic delta forwarding and
endpoint-specific proof checks. No runtime control flow is selected by
replay/record operator name.

Implicit handler-selector normalization applies to the finite publication
profile. Reference startup handlers retain their existing guard path: reference
binding modification and startup activation remain outside the structural delta
profile. Top-level valid plus endpoint-specific modification proves an admitted
structural delta observation; all_valid is not required.

Capability actions and non-clock queries use receiver-first prelude calls
with a direct injected name as the first positional argument; remaining arguments use ordinary
positional/named binding. Dotted capability methods are not aliases. Replay
uses ordinary list length/indexing; every stored timed entry is present.
Bare runtime return terminates without publishing; `return null` is not a
no-output operation. Scalar endpoint `delta_value` proofs remain separate.

Clock observations use read-only properties of the direct injected clock:
`clock.evaluation_time` and `clock.next_cycle_evaluation_time` produce owned
`datetime` values in supported start/evaluation/stop hooks. A local snapshots the
read value. Property invocation, free-function clock aliases, property writes,
unknown properties and non-capability receivers are rejected. `clock.now`
requires wall-clock support absent from this backend and is diagnosed explicitly.
Scheduling remains limited to start/evaluation hooks. Ordinary helpers called
at runtime may request clock/logger services; deterministic wiring service
execution remains outside this backend profile.

Ordinary `global_state` is an ordinary run-wide keyed facility, admitted in
start/evaluation/stop without temporal shape or source/sink role constraints.
Get uses an ordinary concrete expected type, never a key spelling or
an enclosing temporal shape; unconstrained reads are diagnosed. Annotated
locals resolve selected generic bindings. Set takes ordinary scalar or required-field
struct payloads; capabilities, endpoint references, signals and nullable values
do not enter this profile. Stop bodies use checked ordinary statements;
input/output publication and evaluation-only operations remain unavailable.
No new replay or recording representation follows from this facility.

Global-state keys currently admit string literals and resolved const string
parameters. General const expressions (including literal concatenation) and
hook-local keys are not evaluated as keys by this backend; diagnostics identify
that subset limitation. Temporal keys are unsupported by the source profile. Equal
keys have one exact ordinary type throughout the statically assembled plan; known
conflicts are checking errors. Per-node typed entry requirements are emitted for
construction preflight; binding does not initialize an entry. Direct annotated
initializers, returns, assignments and conditions supply get expected types;
nested expression and overload-argument inference are outside this subset.

Initialized runtime `var` bindings of the eleven admitted scalar types are
writable owned locals, including in lifecycle hooks and lifted ordinary value
function bodies. `let`, parameters and `for` bindings remain read-only. Branch
scopes preserve each binding identity across shadowing. Scalar assignments
retain their exact type; local `+=` uses existing addition typing (i64, f64,
str), while cache increments retain their i64 profile. Primitive global get
initializes an owned local, so local mutation never implicitly writes the entry.
Uninitialized locals remain outside this backend subset. No value-type qualifier is introduced.

Runtime hook locals admit finite type-generic ordinary structs with required
primitive, ordinary list or nested struct fields. Constructors require every field once by name and
check exact nominal types. Owning local initialization, constructor retention,
and assignment copy independently (value-mutability, VAL-17). `var` admits
whole-value and nested field replacement; `let` is recursively read-only.
Primitive field `+=` uses existing addition typing. Field projections retain
the root's write authority. Unsupported optional/const-generic/recursive
schemas are diagnosed. Aggregate temporal ports remain outside this slice.

Acceptance: source fixtures executed as emitted Rust through lifecycle hooks;
nested text/value copy independence, mutable field/whole replacement, branch
shadowing, rejected readonly writes, missing/duplicate/wrong-type fields and
nominal mismatch. Existing scalar source and global-state tests remain green.

Imported structs preserve qualified identity and declared layouts. Local names
precede imports; cross-module access requires export, and selected exported
layouts reject unexported reachable struct fields (ADR 0013). Declaration
checks remain lazy in this executable subset: unused schemas are not certified.
All eleven admitted scalar field types are covered by source eval assertions.

The checked constructor preserves supplied source order and declared-field
indices after validating the entire call (`struct-constructor-order.md`).
Backend assembly must not reorder or reexecute its argument expressions.

Typed ordinary struct global gets bind lexical views: `let` is recursively
read-only and `var` is exclusive write-through access. Borrow provenance
survives readonly aliases and aggregate projections, while primitive fields
produce owned values. Exclusive aliases, readonly upgrades and helper escapes
are rejected. `hgl-value-check` checks overlapping entry effects over lexical
blocks after const keys resolve; shadowing does not end a borrow. Distinct
branches and completed blocks release their borrows. No runtime borrow registry
is emitted. Constructor, set and already-owning assignment retain independent
values; assigning through a borrowed var updates its entry after RHS retention.

Acceptance includes configured equal const keys, nested aliases and projections,
read/write conflicts, self-replacement, retained copy independence, missing
aggregate entries and failed replacement preserving the previous value.

Ordinary lists admit exact unbounded/fixed identity, contextual empty literals,
homogeneous constant nonempty literals, len, checked i64 indexed reads and
retained end growth. Indexed replacement and runtime-expression list literals
are rejected. Lists can contain primitive, required-field struct or list values.
Writable indexed projections admit content operations without creating an alias.
Borrowed indexed aggregates inherit entry provenance and lexical authority;
primitive reads are owned. Push evaluates its receiver projection and retains
its item before mutation, including self-source appends.

Concrete ordinary value functions execute directly: checked ValueCall bodies
are emitted as fallible lexical calls inside hooks, without scheduling nodes.
Parameters and prepared ordinary node configuration are recursively readonly.
Owning locals initialized from them copy independently. Deterministic wiring
ordinary operations are evaluated by hgl-value-eval; retained list/struct
configuration is materialized once when a generated node is built. Constant
assertion operation failures fail checking; wiring operation failures are stored
on the plan and reported by generated graph construction before any start.
Unsupported native/capability effects in deterministic wiring remain explicit
backend diagnostics. This is not a claim of complete effectful construction
execution or complete held-value operations on temporal aggregate ports.

Acceptance includes source-executed scalar observations of nested owning and
global lists, fixedness, readonly aliases, self-source push, indexed field
mutation, earlier appends after bounds failure, direct ordinary helpers and
wiring construction, and fresh-run isolation.

Uncontextualized nonempty ordinary literals are explicitly unsupported: the
pinned specification describes constant homogeneous list literals but does not
uniquely define their inferred fixedness. Contextual nonempty literals retain
their expected exact list type. Harness sequence typing is unchanged.

Ordinary generic required-field constructors use hgl-struct-check. Explicit
applications and expected-value contexts feed invariant type inference before
field execution. Concrete specializations use the existing ordinary value,
configuration and global borrow paths. No new generic callable syntax is admitted.

A timed yield anywhere in a temporal function body classifies it as a generator
before phase checks. Generators require an admitted publication output shape and
const-only parameters;
clock/logger are the currently admitted explicit capabilities. State/cache,
lifecycle hooks, output/scheduling injections, for and value-return are rejected.
Configuration values use the existing retained read-only node configuration.
Timed yield checks time as duration/datetime and payload against the exact output
context, then emits the operands in source order. Generator body locals have
unique lexical IDs across nested branches/loops; backend-owned hoisting preserves
shadowing and suspension lifetimes. Direct ordinary helper bodies retain their
separate scopes. Runtime while checks bool, defaults to true, and is rejected in
composition and const value functions without silently changing their phase.

Source typing admits the scalar datetime/duration add/subtract table and duration
negation. Written arithmetic inside a yield operand remains inside that operand;
its checked failure precedes payload evaluation. Implicit duration target
resolution still occurs only after both timed-yield operands succeed.

Function phase/header admission uses hgl-body-check. Direct ordinary helpers may
inject the currently supported clock/logger services in runtime context, including
transitive calls from generators. Their checked effects stay inside ValueCall IR;
no graph scheduling or extra runtime node is introduced. Wiring/constant service
execution and other helper injectables remain explicitly unsupported.

Structural publication frontend (spec60a2d7e): exact finite shapes and derived
ordinary delta types are checked before construction. Sparse delta constructors
use hgl-delta-check prevalidation and ordered IR parts. Scalar payload rules
remain reduced delta types; structural return/out/yield match exact originating
shape. Structural delta_value preserves a readonly evaluation-local observation,
with typed immutable aliases and explicit retention boundaries. hgl-flow-check
owns the unchanged guard proof analysis, now shared across admitted publication
shapes. Neither complete held structural values nor structural-delta inspection,
comparison or temporal payload endpoints are admitted by this extension.

Eval configures ordinary replay values as `list<TimedValue<T>>`: only
present input deltas become absolute timed entries and the dense horizon stays on the
plan. It invokes the normal source replay and record operators, supplies the
recorder's ordinary const key, and binds that exact typed recording before start.
Nested ordinary generic parameter inference applies equally to operator
signatures and their implementations. Replay/record-specific injectable names
are no longer admitted by source checking.

Recorder keys are selected before start against all resolved source requirements,
including unexecuted branches and regardless of type. The current eval entry
creates a fresh store and has no supplied seed or dynamic nested-graph interface;
those source APIs are not implied. Selection adds no reserved string namespace
or per-tick key comparisons. Ordinary caller-selected keys keep normal sharing.

Collection eval normalizes closed delta expressions with their exact parameter
shape and validates each input trace before start through hgl-eval-data. Invalid
membership/empty publications retain a graph-construction error with the input
parameter and zero-based position. Expected values retain exact derived types;
comparison ignores sparse entry ordering but preserves child omission.

Explicit list sizes are normalized through normal checked ordinary expression
resolution and constant evaluation before type formation. Supplied const
configuration is available in signature sizes and local annotations/explicit
constructors; runtime inputs and service effects cannot determine a type.
Nested type arguments retain canonical evaluated size identities. The independent
hgl-type-sizes layer handles nested type traversal and closed scalar size forms.
Endpoint/clock/payload helper checking is extracted to hgl-endpoint-check.

Eval signature candidates use independent checking/evaluation state; only the
selected candidate contributes configuration failures, documentation, and type
normalization caches to graph construction. Rejected candidates cannot poison
the selected graph with an operation failure.

Ordinary return expressions in composition are evaluated before a void result is
discarded, preserving construction failures and ordinary effects.

Finite nonrecursive nominal fields admit non-null scalar literal defaults; contextual calendar defaults remain recipes until run preparation. Complete
ordinary constructors retain supplied fields in written order, then omitted
defaults in declaration order. Sparse deltas never apply those defaults. Generic
specialization checks default types; unsupported non-fixed expressions and null
optionality remain explicit diagnostics.

Test bodies may establish ordinary harness bindings before eval/assert calls.
Check and evaluate each binding once in source order in the test's lexical
ordinary environment; use those values when preparing both inputs and expected
results. Preserve let/var access rules and fresh eval lifecycle per invocation.
Constant list/tuple expressions may reference already evaluated ordinary locals
inside const functions or tests; closed value evaluation, not literal-only IR
shape, determines their compile-time data. Unknown/open values remain errors.

## Temporal scalar preparation

Temporal publication preparation follows the pinned temporal-scalar contract.
Check executable test setup into ordered hgl-harness-ir instead of executing
provider-dependent inputs in the compiler. Type/layout/topology selection remains
compile-time; never fabricate a scalar to specialize a graph. Prepare supplied
constant and temporal arguments in one written order, once for the selected
callable. Ordinary assertions and eval expectations retain their source order.
Provider-dependent node-hook construction remains outside this eval increment;
report unsupported use without adding a source injectable. May use the new
hgl-harness-ir and hgl-rust-preparation boundaries; no budget increase.

Source discovery delegates to hgl-library-files; compiler-only static preparation
provenance delegates to hgl-value-access::StaticValues. Ordinary helper calls
retain supplied values once in written order and bind readonly lexical parameter
locals. Omitted contextual defaults follow supplied arguments. Direct readonly
composition aliases preserve already prepared configuration; new contextual
construction inside composition and node hooks is explicitly unsupported.

## Contextual local bindings

Initialized local checking delegates to hgl-local-check (spec f5ed703).
Composition tracks temporal var authority independently of the port's identity;
compatible rebinding preserves earlier aliases and consumed connections. Scalar
locals remain ordinary, including annotated i64-to-f64 widening. Assignments
cannot cross categories through automatic lifting, even when unused. Temporal
scalar binary expressions use existing checked arithmetic and ordinary nodes;
compound assignments validate the resulting category and type. Node execution
locals retain ordinary ownership and observation rules. Uninitialized declarations
and temporal graph conditionals remain explicitly unsupported; this change does
not claim definite-assignment or dynamic-branch lowering support. Ordinary wiring-time conditionals with connection assignments delegate to
hgl-wiring-locals, retaining only the selected branch's values and binary nodes.
Both branches check fixed categories and lexical authority. This narrow branch
profile admits local declarations, name assignments and scalar/binary expressions;
arbitrary calls remain explicitly unsupported. Existing ordinary-only blocks
retain their broader established execution path.

Validate indexed enum declarations before source checking. Qualified declared
members resolve to exact owning enum literals through hgl-enums. No implicit
integer conversion or additional enum operation is introduced. Enum type calls
report unsupported construction and suggest qualified declared members. May use
hgl-enums.

Scalar-key delta construction uses ParsedLiteral constant callbacks. Exact map
keys and set members may retain provider recipes in cold preparation; fixed
list/tuple indices remain constant i64. hgl-delta-check owns the conversion from
checked parts to typed key/payload IR, preserving source argument order.

Prepared test operations share hgl-static-values::PreparedLexicalScope, retaining
immutable key origins across setup, arguments and expectations. Source checking
proves key eligibility without substituting local initializers into constructors.
