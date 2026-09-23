# Card: compiler tooling spike

Status: experimental comparison; not a production compiler API.

An isolated workspace under `experiments/compiler-tools` tests the owner's
Lexer -> Parser -> Checker -> Rust Emitter split. Dependencies are approved for
this comparison only. Runtime dependencies, code and budgets stay unchanged.

Its small library exports `model::{Kind, Token, Ty, Expr, Decl, Issue, Parsed}`;
`lex::{handwritten, generated, significant}`; `hand::parse` and
`combinator::parse`; `check::{check, instantiate}`; `emit::{text, tokens}`;
`tooling::{lossless, diagnostics}`. Candidate-specific functions are feature
gated. They exchange owned syntax and checked data, not runtime handles.

The [experiment contract](../../experiments/compiler-tools/README.md) defines
the accepted subset, literal expectations and comparison procedure. Preserve
source bytes and report malformed input. Explicit `ref<ref<T>>` is invalid;
substituting `T = ref<U>` into `ref<T>` yields `ref<U>`. Neither emitter checks
HGL types. Both compile and run against the same accepted runtime fixture.

`run.py` tests baseline and candidate paths, compiles each emission independently,
and optionally records cold build and warm execution measurements. No automatic
snapshot acceptance. Compiler semantics, generated Rust and graph fixture
coverage must be reported separately; shared support code is not generated code.
