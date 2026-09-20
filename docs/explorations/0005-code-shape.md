# 0005 — The shape of the code

Status: sketch

A description, not a design to build from yet: no crate below exists. Follows
[decision 0001](../decisions/0001-rust-end-to-end.md); the order of work is in
[0006](0006-build-sequence.md). Every budget is a first guess.

**The right-hand half is built first.** The runtime — `value`, `ts`,
`kernel`, `wiring`, `std` — needs nothing from the compiler: graphs are wired
by hand-written Rust and checked against hgraph's `eval_node` tests. The
compiler crates below are one possible left-hand half; whether they are
written at all, or the existing C++ compiler simply gains a Rust emitter, is
decided once there is a runtime to target (0006, step B1).

## The idea

Two halves joined by data, and four seams that everything passes through.

```text
             FRONT END                          BACK ENDS            RUNTIME
 .hgl ─► syntax ─► sema ─► Graph IR ──┬──► interpreter ──┐
                          (data only) │                  ├─► wiring ─► kernel
                                      └──► emit Rust ────┘   (generated code calls
                                                              the same wiring API)
        shared by both halves: the type model and the operator resolver
```

| Seam | What crosses it | Why it is a seam |
|---|---|---|
| **Graph IR** | a serialisable program: types, contracts, composition ops, node bodies | The only thing a back end may see. It is also the loadable program form the reference lacks ([0002](0002-reference-implementation-profile.md)) |
| **Wiring API** | "add this node, bind this input to that output, register this operator" | The only way a graph gets built — by the interpreter, by generated Rust, by hand-written Rust, later by Python |
| **`Node` + `NodeType`** | behaviour (start / eval / stop) plus policy data (schemas, active and valid inputs, capabilities) | The only way behaviour enters the kernel. Generated, interpreted, native and nested-graph nodes look identical to the scheduler |
| **Provider** | a module's contributions: types, operator contracts, candidates; init and deinit | How the standard library, native packages and compiled HGL modules plug in. Keeps the upstream lifecycle and fingerprint rules |

Each seam gets a written contract and its own tests before the code on either
side of it. The `.hgspec` draft (hgraph PR #796) already sketches three of the
four — its `NodeType`, `GraphType`, `TimeSeriesShape` and `Delta` records carry
over almost unchanged as Rust structs and enums.

## Crates

The crate graph is the layering rule: cargo refuses an import a crate has not
declared, which replaces the reference's grep-based architecture test.

| Crate | Owns | May use (ours) | Third-party | Budget |
|---|---|---|---|---|
| `hgl-types` | Canonical value types and time-series types, interning, temporalisation (`atomic`, `rolling`, sized lists), canonical text | — | — | 2.0k |
| `hgl-resolve` | Operator contracts, candidate patterns, constraint evaluation, ranking, ambiguity and rejection reasons | types | — | 2.5k |
| `hgl-syntax` | Source manager, diagnostics, lexer, parser, lossless tree, AST view | — | — | 4.5k |
| `hgl-ir` | Graph IR and module descriptors: data, readable dump, JSON | types, resolve | serde, serde_json | 2.5k |
| `hgl-sema` | Names and modules, type checking, generics, constraint solving, definite assignment, function classification; typed HIR (private); lowering to Graph IR | syntax, types, resolve, ir | — | 9.0k |
| `hgl-emit-rust` | Graph IR → Rust source and a manifest for the generated crate | ir | — | 3.5k |
| `hgl-value` | Run-time values: scalars, the temporal scalars, tuple/list/set/map/struct, native atomics; equality, hash, order, text, one serial form | types | one datetime/TZDB crate | 3.0k |
| `hgl-ts` | Time-series *data*: value, validity, last-modified and delta for TS, TSB, TSL, TSS, TSD, TSW, REF, SIGNAL; applying and reading deltas. No graph concepts | types, value | — | 4.0k |
| `hgl-kernel` | Graph, `Node`, endpoints and peering, scheduler, clock and modes, lifecycle, node errors, push-source queue, state / cache / injectables, the nested-graph nodes | types, value, ts | — | 6.0k |
| `hgl-wiring` | Operator registry, graph builder, ranking, nested-graph builders, the `eval` record/replay harness | kernel, resolve | — | 3.5k |
| `hgl-interp` | Runs Graph IR: composition ops become wiring calls; one interpreted `Node` evaluates runtime bodies; runs `test` blocks | ir, wiring | — | 3.0k |
| `hgl-std` | Native primitives as a provider, plus the standard library's `.hgl` sources | wiring | — | 3.0k + HGL |
| `hgl` (binary) | `check`, `test`, `run`, `emit`, `dump` | everything above | — | 1.5k |
| `conformance` | Corpus runner and trace comparison (tests only) | hgl | — | 1.0k |
| *later* | `hgl-py` (PyO3), adaptor crates (Kafka, Arrow, web) — the only places `tokio`, `Arc` and `Mutex` may appear | wiring | as needed | — |

About 49k lines of Rust against the reference's 204k + 38k of C++. What the table forbids
matters as much as what it allows: a back end cannot see syntax or HIR; the
kernel cannot see the compiler, the resolver or the registry; nothing under
`hgl-wiring` knows what an operator is.

## Positions to prove

None is settled; each names what it costs and how it gets tested.

**Compiler**

1. **Keep the upstream pass pipeline.** `compiler-architecture.md` is already
   target-neutral: source-accurate syntax → names → types and constraints →
   typed HIR → semantic IR → back ends. Only the last arrow changes.
2. **IRs are arenas of enums with typed ids** — the reference's shape, with
   sum types instead of enum-kind-plus-wide-struct. Deriving `Debug` and serde
   replaces ~3k lines of printers and JSON writers.
3. **Hand-written lexer and recursive-descent/Pratt parser over a lossless
   tree**, from the 287-line EBNF. The contract is upstream's: byte fidelity,
   and malformed input represented, not dropped.
4. **One resolver, used twice.** `hgl-resolve` serves the type checker and
   wiring-time resolution of deferred calls. It must select exactly as
   hgraph's `TypePattern`/`ResolutionMap` do, so it is specified by
   selection vectors recorded from hgraph, not by prose.
5. **The emitter's specification is hand-written Rust.** Before any emitter
   exists, the code it should produce is written by hand against the wiring
   API for a spread of corpus files (0006, step B0). An interpreter for
   scripted runs is optional and comes after, held to parity with it.

**Runtime**

6. *Superseded by [0009](0009-designing-for-speed.md): a tag test and
   16–24 bytes per access is ruled out by the speed requirement. Typed
   value columns replace it; a general `Value` survives only as the erased
   interchange form.*
   **One erased representation.** Time-series data holds `Value` (a closed
   enum with one arm for native atomics), not `Ts<T>` generics. Generated
   code reads and writes through a small typed-accessor trait. One copy of
   the endpoint machinery serves the interpreter, generated code, Python and
   record/replay. *Cost:* a tag check per access and 16–24-byte values where
   the reference plans exact layouts. *Test:* per-tick overhead against the
   reference, in the spike.
7. *Superseded by [0009](0009-designing-for-speed.md): a generation check
   on every read and two moves per evaluation are ruled out. Handles are
   checked at bind time and producers unbind their watchers on disposal;
   the "fallback" below — `unsafe` in one audited crate — becomes the plan.*
   **Engine-owned arenas, typed generational ids, no pointers between
   nodes.** An input stores the id of the output it is bound to; an output
   stores the ids of its subscribers. A node is lifted out of its slot while
   it evaluates, giving it exclusive access to itself and shared access to
   everything else — no `Rc`, no `RefCell`, no `unsafe`. A REF is an id, so a
   removed TSD element or a torn-down switch branch makes it *detectably*
   stale instead of dangling. *Cost:* an indirection per input read.
   *Test:* the same spike. *Fallback:* raw pointers confined to one audited
   crate under Miri.
8. **The engine owns policy, the node owns behaviour.** Activation and
   validity admission come from `NodeType` data, as in hgraph; `Node::eval`
   runs only when admitted. Every hook returns `Result<_, NodeError>`; the
   engine routes it to a captured error output or propagates (ADR 0009).
9. **Single-threaded by type.** A graph is `!Send`; the push-source sender is
   the one `Send` handle, a bounded queue with the queue / conflating / burst
   policies. Clock mode is an enum: simulation, real-time, externally driven.
   The reference needs a counted-mutex test for this; here it does not compile.
10. **Nested graphs are ordinary nodes.** `switch`, `map`, `reduce`, `mesh`,
    `feedback` and `try_except` implement `Node` and own child graphs built
    from a builder captured at wiring time.
11. **No async in the kernel.** Adaptors own their threads or `tokio`
    runtimes and meet the graph only at the push sender and sink callbacks.

**Natives**

12. **No `dlopen` to begin with.** A provider is a Rust crate linked into a
    host binary; `hgl` is a library with a thin `main`, and an application
    with its own natives builds its own host. That avoids the unstable-ABI
    problem and keeps the C ABI for genuinely foreign plugins later. *Cost:*
    scripted runs cannot use a `native fn` that is not already linked in.

## Open questions

- Is `hgl-ts` really separable from the kernel, or does peering leak into the
  delta algebra (REF, TSD key sets)? The spike will say.
- Does this system read hgraph's descriptor format (v5), or define its own
  beside Graph IR? v5 carries `cpp_symbol` and CMake metadata.
- Where do interned types live at run time — a global interner, or one owned
  by the engine? The reference's answer is global, with locks.
- What is the unit of line budget: the crate, or the module within it?
