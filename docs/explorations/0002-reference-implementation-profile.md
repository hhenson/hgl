# 0002 — Profile of the reference implementation

Status: sketch

Measured on `hgraph` main, 2026-09-19 (tracked files only, build trees
excluded). Input to [0001](0001-implementation-language-survey.md): before
choosing a language to make the runtime *compact*, know where the reference's
bulk actually is.

## Size

| Area | Language | Lines |
|---|---|---|
| Runtime core `src/` + `include/` | C++23 | 203,558 |
| — of which `types/` (value, metadata, time-series, wiring) | | ~110,000 |
| — of which `runtime/` (graph, executor, nested nodes, checkpoint, distributed) | | ~37,500 |
| — of which `lib/std` operator library (586 `register_overload` calls) | | ~40,800 |
| Python bridge (nanobind) + in-core Python support | C++ | ~20,000 |
| Python DSL `python/hgraph/` | Python | 84,363 |
| Extensions: web, fabric, persistence, kafka, analytics | C++ | 58,381 |
| Runtime tests | C++ | ~94,500 |
| HGL compiler `language/src/` | C++23 | 38,471 |
| HGL compiler tests | C++ | ~17,900 |
| HGL stdlib + examples + test corpus | HGL | 81 files, ~5,500 |

## Where the runtime's bulk comes from

- **Type erasure built by hand.** Plan / Schema / Ops / Builder / Value / View
  is a sound vocabulary, but in C++ every family needs its ops table, plan
  factory, interning, builder and view spelled out. The layers built this way
  are `value/` ≈ 20k, `metadata/` ≈ 22k and `time_series/` ≈ 27k lines (how
  much of each is erasure plumbing was not measured). 44 `virtual` in the whole
  core against ~2,300 `template` occurrences.
- **Header/implementation duplication.** 99k of the 204k lines are headers.
- **Template-instantiated operator library.** A registration TU has a
  documented 1 GB peak-compiler-memory budget, a tool exists only to police
  it, and families are split into `_impl_<group>.cpp` files to stay under it.
  This is exactly the code HGL is meant to absorb: the architecture doc's goal
  is an HGL standard library that replaces hand-written graph and node
  implementations, "retaining C++ only for the runtime kernel".
- **Scope that is not kernel.** Checkpointing (~3.3k), distributed/spawn
  (~5.8k), services/adaptors/contexts (~6.2k), record/replay + observers +
  profiler (~3.4k), three codecs (binary, JSON, table; ~7k).
- **Build system.** Compiler alone: 602 + 755 + 332 lines of CMake plus 2,196
  lines of CMake test fixtures. `clang-format` is a hard *runtime* dependency
  of `hgl run`, because every scripted path formats, compiles and loads C++.

## What the kernel has to do (the irreducible part)

1. A type-erased **value layer**: scalars, the RFC 0002 temporal scalars,
   tuple/struct (bundle), list, set, map, nominal closed families; equality,
   hash, ordering, string, and one serial form.
2. **Time-series endpoints** over that layer: TS, TSB, TSL (fixed and
   unbounded), TSS, TSD, TSW (tick and duration), REF, SIGNAL. Each with
   validity, modified, last-modified, delta, and output → input peering with
   active/passive subscription and REF rebinding.
3. **Graph, node, scheduler**: rank-ordered single-threaded evaluation,
   per-node scheduler, lifecycle (construct → start → eval → stop → destroy,
   with rollback on partial start), node error model (ADR 0009).
4. **Clock and modes**: simulation, real-time, externally driven. Push sources
   hand off from foreign threads through a bounded queue
   (queue / conflating / burst); the evaluation thread is lock-free per tick.
5. **Nested, dynamic graphs**: switch, map (TSD and TSL), reduce, mesh,
   feedback, try/except — sub-graphs built and torn down at run time.
6. **State, cache, injectables**: recordable `state`, reconstructible
   `cache<T>`, `out`, `logger`, `clock`, `scheduler`.
7. **Operator registry**: nominal operators, candidate patterns, ranking,
   ambiguity and rejection diagnostics. Wiring-time only.
8. **Record/replay harness** sufficient to run `assert eval(...) == [...]`.
9. **A native boundary**: load compiled modules, check a descriptor
   fingerprint, call `init`/`deinit` through a versioned C ABI.

Everything else in the reference is library or integration and can arrive
later as HGL source or as a native package.

## Reusable as-is

- **The language documents** in `hgraph/language/docs` (~18k lines), including
  a 287-line EBNF grammar in `developer-guide/syntax-and-semantics.md`.
- **The `.hgl` corpus**: 81 files with 62 in-language `test` blocks and ~145
  `assert eval(f, in: [...]) == [...]` assertions. These are target-neutral
  tick-level expectations. `tests/codegen/parity.hgl` already holds two
  backends to identical ticks; a new runtime is a third party to that check.
- **The reference runtime as an oracle.** Any program the new runtime accepts
  can be run on the reference for a differential check of ticks.
- **The compiler's IR shape.** Every IR is a flat arena of structs indexed by
  typed `uint32` ids: no class hierarchies, no `virtual`, no `unique_ptr`.
  It ports to any language almost mechanically. The C++ emitter includes no
  hgraph headers — it is a pure IR → text pass.
- **The C lifecycle ABI** (`hgl/native_module_abi.h`, 64 lines) is already C,
  not C++.

## Not reusable

- The **direct-wiring backend** (3.7k lines, 25+ hgraph headers) — it *is* the
  coupling to the reference runtime.
- **Overload resolution**: the language model requires generic matching to
  "lower to hgraph `TypePattern` and `ResolutionMap` rather than a
  language-local matcher". A separate runtime must own an equivalent
  resolver, specified well enough to give identical selections.
- The **module descriptor** (JSON, format v5) is interface-only: signatures,
  types, constraints, layouts — no bodies, statements or blocks. A loadable
  program form does not exist yet; `--dump-hgraph-ir` is a diagnostic text
  dump, explicitly not a compatibility format.
- `native fn` bodies are C++ text over `hgraph::` view types. ADR 0008 already
  concedes that `cpp_type`/`cpp_symbol` "bind the C++ target".
- 17.9k lines of Catch2 compiler tests and 9.7k lines of byte-exact emitter
  snapshots.

## Open questions

- How small can the kernel be? A budget has to be set before specifying, or
  there is nothing to hold generated code against. A first guess from the
  list above: 15–25k lines excluding the operator library.
- Does the new compiler front end start from the existing C++ front end
  (add an IR serialiser, write only the emitter), or is it rewritten? The
  former is cheaper; the latter is the better test of the method.
