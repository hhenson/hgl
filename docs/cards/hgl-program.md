# Card: hgl-program

Link a closed HGL graph against source modules and selected native parts, then
emit Rust for the existing engine. No third-party dependencies. Budget: 2200
source lines. Lexer, declaration indexing/parser, checker/wiring and emitter
remain separate modules.

Public surface: `compile` accepts named sources and an entry name in the first module;
`compile_files` loads explicit files and library roots;
`Program` is an opaque checked graph; `emit_rust` emits it. Sources include
unchanged standard-library files. Index all declarations; check selected bodies
on demand. Unused unsupported library bodies are not advertised as implemented.
A selected unsupported construct, ambiguous candidate or unmatched native part
is an error, never a native fallback.

This slice admits scalar i64/bool temporal endpoints, fixed i64/bool/str/duration
values, generic scalar operator matching with explicit instantiations, imports,
module parts, defaults/named arguments, graph composition, source-only `start { alarm.schedule(fixed_duration) }`, cache,
conditionals and the scalar expressions needed by diagnostic sinks. Fixed graph
arguments are evaluated during wiring and captured in generated node code.
Strings are native formatting temporaries; this does not add TS<str> storage.

HGL owns scheduling, node admission, the cache counter and formatting composition.
Rust providers implement only selected native value signatures. The emitted
module contains generated node implementations and graph construction, without
runtime name lookup on ticks. Preserve selected source documentation.

Acceptance: use the pinned library's actual const and debug_print bodies; source
42 ticks at start and prints `answer: 42` once. Delay 2us shifts that tick by two
cycles. Negative values and independent fresh runs work. Sample 2 suppresses a
single tick. Input tests cover absent and equal ticks and sample counters.
