# 0010 — Rust compiler tooling comparison

Status: measured spike; production dependency choices remain proposed.

The owner selected Lexer -> Parser -> Checker -> Rust Emitter. The
[isolated experiment](../../experiments/compiler-tools/README.md) compares
implementations behind those boundaries. It does not add a production compiler
or change the runtime. The earlier alternative of retaining the C++ front end
is no longer under consideration.

## Correctness

Both lexer/parser paths produce the same syntax and checked shapes for the
bounded corpus. Tests cover source-byte preservation, comments and CRLF,
significant newlines, multiline signatures, adjacent generic closers, precedence,
two-error recovery, unknown names, return mismatches and recursive structures.
A 100-function file checks that neither parser silently drops declarations.

Explicit `ref<ref<T>>` is invalid source. Substituting `T = ref<U>` into
`ref<T>` produces `ref<U>`. Both cases have separate tests. An intervening
collection keeps its reference boundaries. The reference compiler accepts the
ordinary and generic corpus and rejects explicit nesting; there is no deviation
for that rejection. Substitution is tested directly in the small checker, not
through a complete generic call resolver.

Each of the four parser/emitter combinations produces Rust that compiles and
passes the same 14 description tests, including all 9,990 accepted observations,
plus two emission tests. The generated shape replaces only the shape constructor
in the existing nested-graph fixture. Existing handwritten nodes still implement
its switch/map behaviour. A generated arithmetic body also runs through the
engine and matches literal tick expectations checked by the existing HGL
compiler/runtime. These are body/shape emission checks, not full graph lowering.

Rowan round-trips valid and invalid text, but only as a flat token tree here.
Codespan renders both parser errors and an end-of-file error. Neither result
proves a full recovered syntax-tree or semantic diagnostic implementation.

## Measurements

Linux x86-64, Rust 1.98.1, default Cargo profiles. Cold builds use fresh target
directories, cached downloads and disabled compiler wrappers: median of three.
Execution uses release builds: median of seven batches, 2,000 operations per
batch (100 for the 100-function input). Times include allocations and drops;
Chumsky's per-file adapter and grammar construction are included. Reusing a
cached grammar has not been measured.

| Operation | Handwritten | Candidate |
|---|---:|---:|
| Lexer | 0.63 us | Logos 1.17 us |
| Parser, small file | 1.28 us | Chumsky 3.13 us |
| Parser, 100 functions | 30.28 us | Chumsky 74.34 us |
| Two-error recovery | 0.41 us | Chumsky 2.29 us |
| Lexer + parser | 2.15 us | Logos + Chumsky 4.60 us |
| Rust emission, both formatted | text + syn/prettyplease 56.36 us | quote + syn/prettyplease 60.69 us |

Raw text emission alone is 1.72 us; comparing that directly with formatted
emission would charge formatting to only one side. Rowan construction plus
text round-trip is 5.42 us; no competing full syntax-tree implementation was
measured. Parser adapters are 206 versus 137 formatted source lines (handwritten
versus Chumsky), excluding shared source/token/type code and vendor code.

| Standalone build | Debug | Release |
|---|---:|---:|
| Handwritten baseline | 0.35 s | 0.49 s |
| + Logos | 2.50 s | 2.61 s |
| + Chumsky | 2.25 s | 2.77 s |
| + Rowan | 0.71 s | 0.93 s |
| + codespan-reporting | 0.58 s | 0.79 s |
| + quote/proc-macro2/syn/prettyplease | 1.60 s | 3.59 s |

These are implementations of one bounded grammar on one host, not general crate
rankings. The [raw evidence](../../experiments/compiler-tools/results/) includes
all samples, exact dependency/source hashes and macOS/reference validation.
Nine candidate tests pass in debug and release on macOS; candidate validation
and all four generated variants also pass on Linux. Production `cargo xtask ci`
and both existing fixture checks pass on macOS. Hosted checks validate the spike
on Linux and Windows; their current status belongs to the PR, not this report.

## Recommendation

- Start with handwritten lexing and recursive-descent/Pratt parsing. The tested
  custom newline/trivia rules remain necessary with Logos. Chumsky is a viable
  alternative: its grammar is shorter, but its combinator types and recovery
  behaviour require more library-specific knowledge. The small subset does not
  settle how these tradeoffs scale to the complete language.
- Use codespan-reporting for diagnostic rendering, behind our own source IDs,
  byte ranges and diagnostic records. Keep the checker and its type rules ours.
- Keep explicit Rust emission initially; evaluate syn/prettyplease for formatting
  separately from quote-based construction. Both emission approaches passed;
  formatting and Rust syntax validation do not replace compiling generated code.
- Do not select Rowan from the flat-storage result alone. Compare recovered
  declaration/expression trees before choosing the production representation.
  Defer incremental analysis until repeated compilation gives it a concrete job.

The spike excludes strings/temporal literals, imports, calls and named arguments,
constraints, inference, phase checking, native mappings and full graph lowering.
Those remain contract/corpus work before a production implementation. There are
no new runtime performance claims and no increased production line budgets.

Crate APIs checked against primary documentation:
[Logos](https://docs.rs/logos/0.16.1/logos/),
[Chumsky](https://docs.rs/chumsky/0.13.0/chumsky/),
[Rowan](https://docs.rs/rowan/0.17.0/rowan/),
[codespan-reporting](https://docs.rs/codespan-reporting/0.13.1/codespan_reporting/),
[quote](https://docs.rs/quote/1.0.47/quote/),
[proc-macro2](https://docs.rs/proc-macro2/1.0.107/proc_macro2/),
[syn](https://docs.rs/syn/3.0.6/syn/),
[prettyplease](https://docs.rs/prettyplease/0.3.0/prettyplease/).
The experiment lockfile fixes the dependency graph. Its advisory, license,
source and ban checks pass; duplicate syn and hashbrown versions are warnings
under the existing policy, not suppressed findings.
