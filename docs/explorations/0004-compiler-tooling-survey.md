# 0004 — Languages and tools for writing compilers, 2026

Status: sketch

Gathered 2026-09-19 as input to
[0001](0001-implementation-language-survey.md). ✓ marks what was re-read at
source; versions were checked against package registries by a research agent.
No formal survey of "what are new compilers written in" exists.

## What new compilers are written in

GitHub repositories tagged `compiler`, created after 2021, over 50 stars
(self-selected tags, indicative only): **Rust 123**, C++ 59, C 58, Python 48,
TypeScript 36, Go 19, Java 14, C# 12, Kotlin 9, OCaml 6, Zig 6, Haskell 4,
Swift 3.

The migrations of 2025–26 say more than the counts:

| Move | Reason given |
|---|---|
| TypeScript → **Go** (TS 7.0, GA 2026-07) | A port, not a rewrite: pervasive cyclic structures (parent pointers, mutually referring symbols) would force a redesign in Rust; Go is "the lowest-level language that still gives GC" with layout control |
| Roc: Rust → **Zig** ✓ (2026-07) | Rewrite was needed anyway; incremental rebuild 10.0 s → 0.035 s; arenas and struct-of-arrays; "if unsafe is going to be pervasive… choose a language that's safer than Unsafe Rust". They miss Rust's polymorphism and private fields |
| Flow: OCaml → **Rust**; Pyre → Pyrefly (**Rust**) | Windows and cross-platform support, contributor pool, threading |
| Bun: Zig → **Rust** ✓ (2026-05) | Use-after-free, double-free and leak-on-error-path bugs: "In safe Rust, these are compiler errors" |
| Ladybird LibJS: C++ → **Rust** ✓ (2026-02) | Swift's C++ interop "never quite got there" and non-Apple support was limited |

Reading: Rust is the default for new language tooling. A GC'd language wins
when porting a cyclic, mutation-heavy design unchanged. Zig wins when unsafe
code is unavoidable everywhere and rebuild speed dominates. OCaml is losing
industrial ground despite being the best-fitted language on paper.

## Fit for compiler code

| Language | Sum types + exhaustive match | Tree/graph IR memory | Edit–run loop | Ship / embed |
|---|---|---|---|---|
| Rust | excellent | arenas + index ids idiomatic; pointer cycles painful | slow | static binary; C ABI, PyO3 |
| OCaml | best in class | GC, trivial | fast | native exe; awkward to embed; weak Windows |
| Zig | tagged unions, exhaustive `switch`; comptime instead of traits | arenas, SoA, manual | fastest | excellent C ABI; pre-1.0 |
| Go | none (interfaces + type switch, not exhaustive) | GC, easy cycles | very fast | static binary; c-shared drags the runtime |
| C++ | poor (`std::variant`/`visit`; pattern matching slipped to C++29) | manual arenas | slow | native to a C++ host |
| Kotlin / C# / Scala 3 | sealed hierarchies, exhaustive `when`/`match` | GC | medium | weak as a C-ABI library |
| TypeScript | discriminated unions | GC | instant | needs a JS runtime; hit the ceiling that sent `tsc` to Go |
| Haskell | excellent | GC, space leaks | medium | hard to distribute |

The reference compiler already works around C++ here: node kinds are
`enum class` plus a wide struct with unused fields, and passes are large
`switch` statements — sum types written by hand without the exhaustiveness
check.

## Tools, with a verdict for HGL

| Area | State of the art | For HGL |
|---|---|---|
| Parsing | 8 of the top 10 languages use a **hand-written recursive-descent + Pratt** parser: error recovery, diagnostics, speed, reuse by the IDE. rust-analyzer's resilient parser over a lossless `rowan` tree is the reference design for an LSP-ready front end | **Adopt.** The lexer is already hand-written; lexy only covers an 88-production token grammar (1,099 lines), and a 287-line EBNF exists to write from |
| Generators / combinators | ANTLR 4.13 (no release since 2024), Menhir (best LR, OCaml), LALRPOP, pest, chumsky (pre-1.0, recovery-focused), winnow 1.0, tree-sitter (editors, not compilers), lexy / PEGTL (C++), Lark, Langium/Chevrotain (TS) | Skip |
| Diagnostics | Rust alone has several off the shelf: codespan-reporting, ariadne, miette, annotate-snippets | One of them, or ~150 lines by hand — today's format is `path:line:col: category: message` plus a caret |
| LSP | `lsp-server` (rust-analyzer's, synchronous), tower-lsp-server (community fork), async-lsp; Langium generates a server but only for TS hosts | Defer; keep the syntax tree lossless so it stays possible |
| Incremental queries | salsa (0.x, API still moving; used by rust-analyzer and ty) | Defer; whole-module recompiles are fast at this size |
| E-graphs, Datalog | egg, egglog, ascent, Soufflé | Skip; the constraint solver is ~1k lines |
| Language workbenches | MPS, Langium healthy; Xtext at risk; Spoofax 3 experimental. Poor at custom generics and overload resolution | Skip |
| MLIR | Steep: TableGen, slow rebuilds, chasing LLVM. xDSL (Python) is the light route | Skip; nothing in HGL needs its lowering ecosystem |
| Native back ends | LLVM 23, **Cranelift** (a Rust library), QBE, copy-and-patch, Wasm via wasmtime/binaryen | Not needed to emit source or a serialised graph. Cranelift is the only one that embeds as a library without a toolchain — relevant to scripted execution ([0001](0001-implementation-language-survey.md), open question 1) |

## What HGL's compiler actually needs

Sum types and exhaustive matching, arenas with typed ids, a diagnostics
renderer, JSON, snapshot tests, and a text emitter. No parser generator, no
native back end, no IR framework. That rules nothing out, but it removes every
reason to pick a language for its compiler *libraries*: the choice can be
made on the runtime's needs and carried to the compiler for the sake of one
toolchain and shared type definitions.

Sources: <https://github.com/microsoft/typescript-go/discussions/411>,
<https://rtfeldman.com/rust-to-zig>, <https://bun.com/blog/bun-in-rust>,
<https://ladybird.org/posts/adopting-rust/>,
<https://pyrefly.org/blog/lessons-from-pyre/>,
<https://notes.eatonphil.com/parser-generators-vs-handwritten-parsers-survey-2021.html>,
<https://www.typefox.io/blog/xtext-langium-what-next/>.
