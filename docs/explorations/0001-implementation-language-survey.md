# 0001 — Implementation language and tooling for the new runtime

Status: accepted — see [decision 0001](../decisions/0001-rust-end-to-end.md)

Supporting material, one topic each:
[0002 reference profile](0002-reference-implementation-profile.md) ·
[0003 AI-codegen evidence](0003-ai-codegen-evidence.md) ·
[0004 compiler tooling](0004-compiler-tooling-survey.md)

## The idea

Accept HGL as specified in `hgraph/language`, and build a new emitter and a
compact runtime with whichever language and tools give an AI agent the best
chance of producing clean, compact, understandable code — while still being
able to lean on existing frameworks for Kafka, Arrow, dataframes, async I/O,
HTTP and Python.

Recommendation: **one language and one toolchain for compiler, runtime and
generated code — Rust — with a C ABI at the plugin boundary and PyO3 for
Python.** No parser generator, no LLVM, no MLIR. And before any of it, a
language-neutral conformance corpus: that, not the language, is what every
successful agent build had, and it keeps this decision reversible.

## What it looks like

| Layer | Proposed | Reference today |
|---|---|---|
| Compiler front end | Rust. Hand-written lexer and recursive-descent/Pratt parser from the EBNF; arena IRs with typed ids (the existing shape) as enums with exhaustive `match`; `serde` JSON; snapshot tests | C++23, lexy, enum-kind + wide structs, ~3.2k lines of printers |
| Runtime kernel | Rust, `std` plus a datetime/TZDB crate (e.g. `jiff`, which speaks RFC 9557). Traits for the erased layers, generics for the typed fast path, graph-owned arenas | C++23; hand-built Plan/Schema/Ops/Builder tables; ~204k lines |
| Generated code | Rust source, built by cargo into the application (AOT). Scripted path open — question 1 | C++ text, `clang-format`, compile, `dlopen` |
| Native escape | a `rust(...) { }` projection beside `cpp(...) { }`, through ADR 0008 target mappings, landed in hgraph first | `cpp(...) { }` only |
| Plugin boundary | C ABI in the shape of `hgl_native_module_v1`; Arrow C Data Interface for bulk data | same C ABI |
| Python | PyO3 + maturin, one abi3/abi3t wheel per platform | nanobind + scikit-build-core + CMake + Conan |
| Integrations | `rdkafka`, arrow-rs / Polars / DataFusion, `tokio` (adaptor crates only, never the kernel), `axum`, `tracing` + OpenTelemetry | librdkafka, Arrow C++, asio/Boost, nghttp2, libcurl — 58k lines of extension C++ |
| Build | one cargo workspace; the crate graph *is* the layering rule | CMake + Conan; 3.9k lines of CMake for the compiler alone |
| Gates | rustfmt; a clippy deny-list; `#![forbid(unsafe_code)]` everywhere but one small crate run under Miri; `cargo-deny`; a line budget per crate; the `.hgl` corpus; differential ticks against the reference | clang-format, an architecture grep test, ratchet counts |

```hgl
native fn increment(value: f64) -> f64 {
    rust(value: f64) { value + 1.0 }
}
```

*Proposed, illustrative only.* Today the only projection is `cpp(...)`, and
ADR 0008 leaves target-mapping syntax open.

## Why

### Five gates, from the brief and the reference

1. No GC pauses and control of layout on the per-tick path.
2. First-class Python embedding, including callbacks into user Python.
3. Mature libraries for Kafka, Arrow/dataframes, async I/O, HTTP/WebSocket.
4. macOS arm64, Linux x86_64 and Windows from one build description.
5. Mainstream and stable enough that agents write it from memory.

| | 1 latency | 2 Python | 3 ecosystem | 4 three OSes | 5 agent fluency |
|---|---|---|---|---|---|
| **Rust** | ✓ | ✓ PyO3 | ✓ | ✓ cargo | ✓ best repository-level results |
| **C++23** | ✓ | ✓ nanobind | ✓ | ✓ at a price | ~ fluent, but worst repository-level results |
| Zig | ✓ | ✗ | ✗ no Arrow, Kafka via C | ✓ | ✗ pre-1.0, churning, thin in training data |
| Go | ✗ GC | ✗ cgo + GIL | ~ | ✓ | ✓ |
| OCaml 5 | ~ needs OxCaml fork | ✗ | ✗ | ~ | ✗ |
| JVM | ~ garbage-free style only | ✗ JPype/jpy | ✓ | ~ | ✓ |
| .NET | ~ | ✗ | ~ | ✓ | ✓ |
| Swift | ~ ARC traffic | ✗ | ✗ | ~ | ~ |
| Mojo 1.0 | ? | ~ | ✗ | ✗ | ✗ |

Only two pass. Every nearby system agrees: per-tick engines are C++ (csp,
kdb+) or OCaml inside Jane Street's own ecosystem (Incremental); nearly every
stream engine started since 2019 is Rust (Materialize, RisingWave — after
abandoning C++ at seven months — Feldera, Arroyo, Pathway, Polars). Feldera is
the closest compiler precedent: SQL compiled to Rust source and linked
through cargo.

### Rust against C++, on what this experiment is about

| | C++23 | Rust |
|---|---|---|
| Agent evidence ([0003](0003-ai-codegen-evidence.md)) | Bottom of SWE-bench Multilingual (28.6%). Memory-safety bugs in nearly every agent-written program in the one controlled study. UB compiles and runs | Top of the same benchmark (58.1%). Chosen by every large agent build. Does nothing about bloat |
| Header / source split | 99k of 204k lines are headers; doubles the context an agent must hold | none |
| Type erasure | Ops tables, plan factories, builders and interning written by hand throughout the value, metadata and time-series layers (~70k lines between them) | `dyn Trait` *is* a compiler-generated struct of function pointers taking the object first; the typed path is a generic |
| Sum types | enum-kind + wide struct, non-exhaustive `switch` | enums; adding a time-series kind makes the compiler list every site to update |
| Operator library | 1 GB-per-TU compile budget, policed by a tool | moves to HGL source, which is the language's own goal; monomorphisation still costs build time |
| Build | CMake + Conan + scikit-build; the usual place agent mess accumulates | `Cargo.toml`; a crate cannot import what it does not declare |
| Rules held in prose today | "tick path lock-free and `shared_ptr`-free" (a counted-mutex test); "borrowed iterators cannot escape an evaluation"; "View borrows, Value owns"; "destroy constructed slots after a failed `start`" | graph types are `!Send`, a push sender is the only `Send` handle; lifetimes; `&` vs owned; `Drop` |
| Existing assets | keeps the 38k-line front end, nanobind bridge and extensions | keeps the docs, the corpus, the IR shape and the C ABI |

The "rules held in prose" row is the argument specific to this experiment. The method
being designed has to turn descriptions into checks
([0003](0003-ai-codegen-evidence.md): correctness can be specified through
oracles, cleanliness has to be enforced). Rust moves a slice of the design —
ownership, threading, lifetimes, exhaustiveness, layering — out of prose an
agent may ignore and into a compiler it cannot.

### Scores, for the record

1 poor – 5 best. The ranking comes from the gates, not from a sum.

| | Agent reliability | Compact compiler code | Runtime fit | Ecosystem | Python | Toolchain | Maturity |
|---|---|---|---|---|---|---|---|
| Rust | 4 | 5 | 4 | 5 | 5 | 4 | 5 |
| C++23 | 2 | 2 | 5 | 5 | 5 | 2 | 5 |
| Zig | 2 | 4 | 5 | 2 | 2 | 3 | 1 |
| Go | 4 | 2 | 2 | 3 | 1 | 5 | 5 |
| OCaml 5 | 2 | 5 | 3 | 1 | 2 | 2 | 3 |
| JVM (Kotlin/Java) | 4 | 4 | 3 | 4 | 2 | 3 | 4 |
| .NET | 3 | 4 | 3 | 3 | 2 | 4 | 4 |
| Swift | 2 | 4 | 3 | 2 | 1 | 3 | 3 |

## What it costs

- **The ownership model must be designed up front.** Outputs know their
  subscribers, inputs know their output, REF rebinds at run time, nested
  graphs are built and torn down while bound to outer outputs. In Rust that
  is graph-owned arenas addressed by ids, or raw pointers inside one audited
  crate. This is the main technical risk and deserves a spike (question 3).
  It is also the design decision C++ lets a code base never make.
- **Agents take Rust shortcuts**: `.clone()`, `unwrap`, `Rc<RefCell<_>>`,
  `Arc<Mutex<_>>`, `unsafe`. Bun's port kept ~13,000 `unsafe` sites. These
  have to be lint errors from the first commit, not review comments.
- **Build latency.** Scripted `hgl run` cannot cheaply shell out to cargo;
  it needs an interpreter for runtime bodies or an embedded JIT. (The
  reference's scripted path already needs clang *and* clang-format installed.)
- **No stable Rust ABI.** Separately built plugins cross a C ABI; generic code
  cannot cross it. AOT applications avoid the problem by linking statically.
- **Less is reused.** The C++ front end, the nanobind bridge and 58k lines of
  extension C++ stay behind. Most extension behaviour exists as crates;
  anything that does not is wrapped through the C ABI or `cxx`.
- **`native fn` bodies are per target.** `stdlib/hgl/hgraph/native.hgl` is
  C++ text over `hgraph::` views. ADR 0008's target mappings stop being
  future work and become the first extension to land in hgraph.
- **The experiment is confounded.** Language and method change together, so
  an improvement cannot be attributed to either alone.
- **Reviewer fluency.** "Understandable" means understandable to its owner.
  If Rust is not comfortable reading, that outweighs the table above.

## Alternatives

- **C++23 again, under constraints** — the only alternative worth taking
  seriously. Choose it if reusing the front end and extensions, or reviewing
  in C++, matters more than the agent evidence; it also isolates the method
  as the only variable. It needs sanitizers in every build, a compile after
  every change, a banned-constructs lint, and a line budget. The header
  split, CMake and hand-built erasure stay: the material the mess was made of.
- **Zig** — the best fit on paper for a compact kernel: comptime instead of
  templates, tagged unions, allocators, 35 ms rebuilds, a trivial C ABI.
  Rejected for now: pre-1.0 with breaking releases, thin in model training
  data, no Arrow, Kafka only through C, Python bindings stuck two versions
  back, and Bun left it over use-after-free bugs. Roc's move *to* Zig rested
  on unsafe code being pervasive, which a single-threaded arena kernel should
  not need. Revisit after 1.0; ADR 0008 already names it as a target.
- **Go** — the most reviewable agent output and the fastest loop. Rejected:
  GC, no sum types, no plugin loading on Windows, Python embedding via cgo.
  TypeScript chose it to *port* a cyclic design unchanged; this is a redesign.
- **OCaml** — the best compiler language and the closest semantic prior art
  (Incremental). Rejected: thin ecosystem, awkward to embed, weak Windows,
  layout control needs a fork, and industry is leaving it (Flow, Pyre).
- **JVM / .NET** — rejected: allocation-free style is non-idiomatic, native
  nodes cost an FFI crossing per tick, Python embedding is second-class.
  Truffle's free JIT is tempting, but Oracle has detached GraalVM from the
  Java release train.
- **Swift, Mojo, Carbon, Nim, D, Odin, Julia** — rejected on ecosystem or
  maturity; see the gate table.
- **Split stacks** (Rust or OCaml compiler over a C++ or Zig runtime) —
  rejected. [0004](0004-compiler-tooling-survey.md) finds nothing the compiler
  needs from a language's compiler libraries, so a split only buys two
  toolchains, two idioms for agents to hold, and no shared type definitions
  for the resolver that compiler and runtime must agree on.

## Open questions

1. **How do runtime bodies execute in scripted mode?** Compile generated
   Rust, interpret a small typed body IR, or JIT through Cranelift (a Rust
   library, no toolchain needed). Leaning: AOT emits Rust, scripted
   interprets, and the parity corpus holds the two to identical ticks — as
   `tests/codegen/parity.hgl` does for the two C++ back ends today. This is
   the next decision and it shapes the emitter.
2. **Does the front end start as a port of the C++ one, or from the EBNF and
   the design docs?** The first is cheaper and has an oracle (`--dump-*`
   output); the second is the truer test of the method.
3. **Arena ids or audited pointers for the endpoint graph?** Spike before the
   specification hardens: TSD + REF rebinding + switch teardown in a few
   hundred lines, per-tick overhead measured against the reference.
4. **What is the kernel's line budget?**
   [0002](0002-reference-implementation-profile.md) guesses 15–25k.
5. **Where does the operator resolver live**, and how is it specified tightly
   enough to select identically to hgraph's `TypePattern`/`ResolutionMap`?
6. **Is the `.hgspec` runtime-contract draft (hgraph PR #796) the starting
   point** for describing the runtime? It already carries conformance traces
   and C, C++, Rust and Swift mapping notes.
7. **Corpus first?** ~145 `assert eval` assertions is thin next to the suites
   in [0003](0003-ai-codegen-evidence.md). Growing it — and recording the
   reference runtime's ticks for every case — may be the first real work,
   whatever language is chosen.

Sources for the gate table and prior art: <https://pyo3.rs/main/free-threading>,
<https://nanobind.readthedocs.io/en/latest/free_threaded.html>,
<https://github.com/Point72/csp>, <https://github.com/feldera/feldera>,
<https://risingwave.com/blog/building-a-cloud-database-from-scratch-why-we-moved-from-c-to-rust/>,
<https://blog.janestreet.com/introducing-incremental/>,
<https://ziglang.org/download/0.16.0/release-notes.html>,
<https://pkg.go.dev/plugin>, <https://oxcaml.org/documentation/>,
<https://blogs.oracle.com/java/detaching-graalvm-from-the-java-ecosystem-train>,
<https://www.modular.com/blog/mojo-open-source>.
