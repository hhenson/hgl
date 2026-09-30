# Card: hgl-program

Check closed HGL graphs and tests against source libraries; emit Rust for the
existing engine. Uses `hgl-source`, `hgl-library`, `hgl-documentation`. Budget:
2200 source lines. No third-party dependencies.

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

Acceptance: the unchanged pinned standard library's 82 tests/128 evaluations,
plus empty/silent/equal ticks, delayed output, fresh state, helper isolation,
wrong values/lengths and propagated node errors. `cargo xtask ci` runs these in
debug and release. The original const/debug graph regressions remain.

Node-scoped `replay_input` and `capture` are non-value capabilities. The first
requires a scalar source; the second an outputless sink with one scalar input.
They cannot escape or appear in value/composition bodies. The checker enforces
ADR0016's exact method names, positional/named arguments, result types and
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
structural instances are diagnosed until a contextual delta contract is admitted.
Formal `signal` parameters retain their signal identity even when the producer
has a scalar payload; they are excluded from both delta access and capture binding.
Only runtime evaluation can read delta metadata. Endpoint identity and proof of
both valid and modified are required; copied payloads/consts are rejected.
Handler and local short-circuit/conditional facts establish those guarantees.
`delta(input)` is not an accessor intrinsic; `delta<T>(...)` is a constructor.

Acceptance also includes mutated source operator bodies proving execution of the
selected HGL handlers, missing-binding construction failure before start, native
start failure, capability method/phase/type/escape errors, translated buffer
errors through generated nodes, generic delta forwarding and endpoint-specific
proof checks. No runtime control flow is selected by replay/record operator name.

Implicit handler-selector normalization is applied to the scalar-input profile.
Existing structural guard emission is retained: applying scalar normalization to
reference startup handlers exposes an unresolved mismatch between reference
binding modification time and startup activation. Structural delta metadata and
that normalization/runtime integration remain outside this completed profile.
