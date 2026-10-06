# 0011 — Crate boundaries: what the split had become, and what a crate is for

Status: measured; the layout is adopted by [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md)

## What was measured

On 2026-10-06, at `d2d1548`, the workspace held 124 packages for 31,441
lines of code (blank and comment lines excluded). The median crate was
under 200 lines; 51 crates were under 150 lines and 51 had at most one
consumer in the workspace. The smallest were `hgl-observation` (19 lines),
`hgl-rust-preparation` (36), `hgl-library-files` (37), `hgl-scalar-keys`
(39) and `hgl-rust-source-slots` (39). No crate had a third-party
dependency, and no crate but the test allocator used `unsafe`.

The split was not architecture. It followed from two rules together: a
crate's `line-budget` is never raised to pass a build, and a card is written
per crate. When a slice needed more code than the nearest crate's budget
allowed, the only move that kept the gate green was a new card and a new
crate. Each slice then added crates instead of lines: six on 2026-09-20,
three on 09-21, five on 09-22, and about a hundred between 09-24 and 10-06.

Three things showed the boundaries were cutting through one design:

- Re-export chains. `hgl-store` re-exported `Kind` from `hgl-bindings`,
  which re-exported it from `hgl-endpoints`, which aliased it from
  `hgl-types`. A consumer saw one API assembled from four manifests.
- Commits crossed crates. Of the first 25 source commits, 16 touched two or
  more crates.
- The compiler wrote crate paths into generated programs
  (`hgl_harness_ir::`, `hgl_rust_ir::`, `hgl_value_eval::`), so a generated
  test program depended on twelve crates, six of them compiler internals.

Compile time gave no reason for the split either way: a cold debug build of
the whole workspace took under a second on the validation host, and
touching `hgl-types` rebuilt everything in a quarter of a second.

## What a crate gives that a module does not

A Rust crate is the compilation unit and the unit of external API. A module
gives everything the cards ask for: private by default, `pub(crate)` for
in-crate sharing, `#![forbid(unsafe_code)]` as an inner attribute that
cannot be overridden below it, and `#[cfg(test)]` tests that reach private
items. What only a crate gives:

1. A separate program: a binary, or tooling never linked into the product.
2. A different compiler policy: `unsafe` allowed, another lint table, Miri.
3. A dependency direction the compiler enforces between two layers that are
   each large enough for the enforcement to pay for the ceremony.

The cards' "may use" lists and budgets do not need crates: a gate can hold
them per module directory (`cargo xtask ci` does, since this change).

## The layout adopted

Fourteen packages. Sizes are code lines at the time of the move.

| Crate | Modules folded in | Lines | Reason it is a crate |
|---|---|---|---|
| `hgl-types` | growing-range, node-error, time-values, window-types, calendar | ~800 | The vocabulary both programs share |
| `hgl-store` | scalar-copy, value-lists, columns, global-arena, global-value, prepared-value, list-storage, recursive-value, list, optional, global, keys, member-table, endpoints, binding-build, bindings, shapes, atomic, rolling, observation, store-build, prepared-store | ~5,700 | The one crate [decision 0003](../decisions/0003-unsafe-confined-to-the-store.md) allows `unsafe` |
| `hgl-kernel` | deadlines, nested | ~890 | Execution |
| `hgl-describe` | plan | ~1,000 | Description and instantiation, the emitter's target |
| `hgl-stdlib` | native, std-native | ~620 | What generated programs link against |
| `hgl-source` | lex, diagnostics, type-shape, nominal-batch, type-syntax, literals, time-context | ~1,900 | Tokens, syntax and diagnostics of the compiler |
| `hgl-semantics` | library, the `*-check` crates, the `value-*` crates, struct and family types, eval-data, harness, harness-ir, rust-ir as `ir` | ~8,000 | The program model and its checks; the IR the backend reads |
| `hgl-rust` | every `hgl-rust-*` crate but `rust-ir` | ~8,000 | The Rust backend; must not be reached by the model |
| `hgl-program` | documentation, library-files | ~2,400 | Linking and emission orchestration |
| `hgl-compiler` | reject, test-annotations, test-units, test-runner | ~2,000 | The `hglc` program |
| `hgl-testkit` | proto-nodes | ~800 | Test support, never in the product |
| `hgl-alloc-count` | — | 39 | Needs `unsafe` and its own lint table |
| `hgl-bench` | `bench/twin` and `hgl-fixed-bench`, as two binaries | ~540 | Benchmark programs |
| `xtask` | — | ~450 | Tooling |

Two boundaries were kept on purpose. The compiler must not reach the
runtime: `hgl-source`, `hgl-semantics`, `hgl-rust` and `hgl-program` depend
on `hgl-types` and nothing else of the runtime. The backend must not be
reached by the model: `hgl-semantics` does not depend on `hgl-rust`.

## Open

- Generated programs still depend on `hgl-source` and `hgl-semantics`, for
  `harness`, `harness_ir`, `ir`, `value_eval` and `time_context`. The
  runtime-facing parts of those modules belong in a crate the runtime side
  owns, so a generated program links against runtime crates only.
- `hgl-compiler` carries its own `lex`, `parse`, `check` and `emit` modules
  beside `hgl-source` and `hgl-program`. Whether they are one compiler or
  two is a question for the compiler review, not for this record.
