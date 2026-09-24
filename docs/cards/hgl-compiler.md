# Card: hgl-compiler

First production slice: compile the upstream const/debug example, not a fixed
string template. No external dependencies. Budget: 1400 source lines, including
the CLI. Lexer → Parser → Checker → Rust Emitter remain separate modules.

Public surface:

- `Source { name: String, text: String }`: one source file or selected part.
- `Diagnostic { source: String, start: usize, end: usize, message: String }`:
  owned byte spans; CLI renders file, line, column and message.
- `CheckedModule`: opaque checked program, with no runtime handles.
- `check(&[Source]) -> Result<CheckedModule, Vec<Diagnostic>>`.
- `emit_rust(&CheckedModule) -> Result<String, Vec<Diagnostic>>`: generated native `Native` trait, node
  structs and graph builder functions. The consumer supplies `crate::Provider`
  implementing that trait. This is the selected library provider, not a runtime
  name lookup. CLI `hglc check FILE --part FILE` and
  `hglc emit-rust FILE --part FILE --out FILE` share checking.

Grammar: module/part headers, exported temporal fn, native const fn declarations
and empty selected implementations, i64/void results, ordinary and const i64
parameters, integer/name/call expressions, graph let bindings, start scheduling
with literal 0s, inject scheduler, bare/scheduled when, return and call statements.
Preserve newlines/comments and spans. Unsupported forms are diagnostics, never
ignored. Public names that cannot be represented by this Rust binding are
emission errors. Names, function order and integer constants must not be hardcoded.

Checker: bare native declarations can be checked; emission requires selected
implementations. Match selected parts to signatures exactly; resolve calls to declaration
indices; reject duplicates, unknown names, wrong arity,
wrong value kinds, unavailable scheduler and recursion. Graph calls construct
nodes; native value calls in handlers remain direct calls. Bare when means any
modified input and all inputs valid; no-input bare when is false. Scheduled when
requires the node's own alarm. Start schedules, never publishes. The emitter
consumes resolved calls and checked shapes rather than resolving names again.

Generated nodes use existing Node/Ctx/Buildable and graph descriptions. Keep
scalar configuration separate from input handles. No per-tick lookup or graph
construction. Each source/graph call creates an independent instance. The native
print helper may allocate/perform I/O; no new runtime performance claim.

Done: upstream HGL parses, emits and executes on hgraph before Rust implementation;
reasoned/Python/C++ const/sink cases agree. Generated Rust compiles independently
and executes the same cases, including source timestamps, sink output, idle
cycles and fresh runs. CLI rejects malformed/unsupported input without output.
No checkpoint, generic/collection lowering, arbitrary expressions or full native
lifecycle ABI claims. Tests exercise renamed functions, changed literals and
multiple calls so the compiler cannot merely recognise the example text.

Mutants on a copy: delay start scheduling; make bare no-input when true; suppress
equal sink values; confuse scalar and temporal arguments; omit signature matching.
