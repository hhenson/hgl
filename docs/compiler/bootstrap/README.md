# Const → debug_print

Reasoning precedes code: the source schedules one evaluation at start, publishes
its configured integer, and the active valid sink prints once in that cycle.
No reschedule means no second tick. Missing inputs do not admit the sink;
equal ordinary ticks still print. A fresh execution has fresh nodes.

`cases.json` holds literal expectations. Released Python and C++ observations
agree in `python.json` and `cpp.json`; neither comparison changes expectations.
Both eval_node harnesses return None for an entirely invalid stream; replay
expands that to absent cells over the supplied input interval. Debug-print
prefixes vary; only integer payloads are compared. There are no deviations.

The shared HGL source is the upstream `language/examples/const-debug.hgl`.
It uses `const_` because `const` is reserved. HGL owns both nodes and composition;
only the print helper is native. hgraph owns its C++ implementation; the Rust
part and provider live in `examples/const-debug` here. This first i64 sink prints
an integer plus a newline; labels, generic formatting and the complete debug_print API are later
work. Native temporal providers and raw-TS helper access are not required.

Acceptance: check and emit have no effects; compile emitted Rust, run the full
graph, observe one source tick and one print at MIN_START, and no future alarm.
Repeat with a fresh instance. Exercise the sink with missing/equal/negative ticks.
Reject unknown names, bad arity/types, scalar/temporal misuse, unmatched or
missing target parts, unsupported syntax and duplicate declarations.

`tools/compiler_mutants.py --output <file>` tests a disposable copy. Five
compiled mutations fail their intended tests: delayed source publication,
no-input bare-handler admission, temporal configuration, signature mismatch,
and suppressed equal printing. The restored baseline passes; see `mutants.json`.

The generated-code test also changes names/constants and nests two source/sink
calls. Both sources publish independently; consumers follow ranked dependency
order, not the textual order of sink calls.
