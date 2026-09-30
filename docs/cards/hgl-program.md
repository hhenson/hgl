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
requirements, defaults, contextual result inference, graph composition, const
lifts, ordered guarded handlers, state/cache, scalar input activity, source
alarms, reference capture/following, and bool/i64 set membership. Set bodies
mutate `out`; `elements(input, added)` supplies typed elements. State resets
for each fresh graph; checkpoint recovery is outside this slice.

`eval` compiles replay nodes, the selected HGL bodies and a recorder. Dense
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

Acceptance: the unchanged pinned standard library's 45 tests/84 evaluations,
plus empty/silent/equal ticks, delayed output, fresh state, helper isolation,
wrong values/lengths and propagated node errors. `cargo xtask ci` runs these in
debug and release. The original const/debug graph regressions remain.
