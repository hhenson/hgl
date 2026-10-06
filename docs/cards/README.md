# Contract cards

Status: accepted for the first slice, 2026-09-19 — revised as each build finds what a card left out

A card is the one page an agent is given to build one crate. It is written by
us, not generated. With it the agent gets the rules of the
[runtime specification](https://github.com/hhenson/hgraph_spec/blob/main/runtime/overview.md) the card names, and the
failing cases and benchmarks the card names — not the whole repository.

This is the method under test
([0006](../explorations/0006-build-sequence.md), "How each crate gets
built"): card → agent → a reviewer with fresh context who sees only the diff
and the card → the owner. When something comes out wrong, the card, a lint, a
budget or a test is fixed, and the work is regenerated.

## Where each card lives now

[Decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) folded the
crates below into modules of fourteen packages. A card still describes one unit
of work; the unit is now a module directory, and `cargo xtask ci` holds its
budget and "may use" list from the crate's `[package.metadata.hgl.modules]` table.

| Card | Now | Of |
|---|---|---|
| `hgl-bench-twin` | binary `twin` | `hgl-bench` |
| [hgl-atomic](hgl-atomic.md) | module `atomic` | `hgl-store` |
| [hgl-binding-build](hgl-binding-build.md) | module `binding_build` | `hgl-store` |
| [hgl-bindings](hgl-bindings.md) | module `bindings` | `hgl-store` |
| [hgl-body-check](hgl-body-check.md) | module `body_check` | `hgl-semantics` |
| [hgl-calendar](hgl-calendar.md) | module `calendar` | `hgl-types` |
| [hgl-collection-check](hgl-collection-check.md) | module `collection_check` | `hgl-semantics` |
| [hgl-collection-values](hgl-collection-values.md) | module `collection_values` | `hgl-semantics` |
| [hgl-columns](hgl-columns.md) | module `columns` | `hgl-store` |
| [hgl-composite-keys](hgl-composite-keys.md) | module `composite_keys` | `hgl-semantics` |
| [hgl-deadlines](hgl-deadlines.md) | module `deadlines` | `hgl-kernel` |
| [hgl-delta-check](hgl-delta-check.md) | module `delta_check` | `hgl-semantics` |
| [hgl-diagnostics](hgl-diagnostics.md) | module `diagnostics` | `hgl-source` |
| [hgl-documentation](hgl-documentation.md) | module `documentation` | `hgl-program` |
| [hgl-endpoint-check](hgl-endpoint-check.md) | module `endpoint_check` | `hgl-semantics` |
| [hgl-endpoints](hgl-endpoints.md) | module `endpoints` | `hgl-store` |
| [hgl-enums](hgl-enums.md) | module `enums` | `hgl-semantics` |
| [hgl-eval-data](hgl-eval-data.md) | module `eval_data` | `hgl-semantics` |
| [hgl-family-types](hgl-family-types.md) | module `family_types` | `hgl-semantics` |
| [hgl-family-values](hgl-family-values.md) | module `family_values` | `hgl-semantics` |
| [hgl-fixed-bench](hgl-fixed-bench.md) | binary `fixed` | `hgl-bench` |
| [hgl-flow-check](hgl-flow-check.md) | module `flow_check` | `hgl-semantics` |
| [hgl-global](hgl-global.md) | module `global` | `hgl-store` |
| [hgl-global-arena](hgl-global-arena.md) | module `global_arena` | `hgl-store` |
| [hgl-global-value](hgl-global-value.md) | module `global_value` | `hgl-store` |
| [hgl-growing-range](hgl-growing-range.md) | module `growing_range` | `hgl-types` |
| [hgl-harness](hgl-harness.md) | module `harness` | `hgl-semantics` |
| [hgl-harness-check](hgl-harness-check.md) | module `harness_check` | `hgl-semantics` |
| [hgl-harness-ir](hgl-harness-ir.md) | module `harness_ir` | `hgl-semantics` |
| [hgl-inheritance](hgl-inheritance.md) | module `inheritance` | `hgl-semantics` |
| [hgl-keys](hgl-keys.md) | module `keys` | `hgl-store` |
| [hgl-lex](hgl-lex.md) | module `lex` | `hgl-source` |
| [hgl-library](hgl-library.md) | module `library` | `hgl-semantics` |
| [hgl-library-files](hgl-library-files.md) | module `library_files` | `hgl-program` |
| [hgl-list](hgl-list.md) | module `list` | `hgl-store` |
| [hgl-list-storage](hgl-list-storage.md) | module `list_storage` | `hgl-store` |
| [hgl-literals](hgl-literals.md) | module `literals` | `hgl-source` |
| [hgl-local-check](hgl-local-check.md) | module `local_check` | `hgl-semantics` |
| [hgl-member-table](hgl-member-table.md) | module `member_table` | `hgl-store` |
| [hgl-name-check](hgl-name-check.md) | module `name_check` | `hgl-semantics` |
| [hgl-native](hgl-native.md) | module `native` | `hgl-stdlib` |
| [hgl-nested](hgl-nested.md) | module `nested` | `hgl-kernel` |
| [hgl-node-error](hgl-node-error.md) | module `node_error` | `hgl-types` |
| [hgl-nominal-batch](hgl-nominal-batch.md) | module `nominal_batch` | `hgl-source` |
| [hgl-observation](hgl-observation.md) | module `observation` | `hgl-store` |
| [hgl-optional](hgl-optional.md) | module `optional` | `hgl-store` |
| [hgl-plan](hgl-plan.md) | module `plan` | `hgl-describe` |
| [hgl-prepared-store](hgl-prepared-store.md) | module `prepared_store` | `hgl-store` |
| [hgl-prepared-value](hgl-prepared-value.md) | module `prepared_value` | `hgl-store` |
| [hgl-proto-nodes](hgl-proto-nodes.md) | module `proto_nodes` | `hgl-testkit` |
| [hgl-publication-trace](hgl-publication-trace.md) | module `publication_trace` | `hgl-semantics` |
| [hgl-recursive-types](hgl-recursive-types.md) | module `recursive_types` | `hgl-semantics` |
| [hgl-recursive-value](hgl-recursive-value.md) | module `recursive_value` | `hgl-store` |
| [hgl-reject](hgl-reject.md) | module `reject` | `hgl-compiler` |
| [hgl-rolling](hgl-rolling.md) | module `rolling` | `hgl-store` |
| [hgl-rust-capacity](hgl-rust-capacity.md) | module `capacity` | `hgl-rust` |
| [hgl-rust-checked-data](hgl-rust-checked-data.md) | module `checked_data` | `hgl-rust` |
| [hgl-rust-collections](hgl-rust-collections.md) | module `collections` | `hgl-rust` |
| [hgl-rust-composite-keys](hgl-rust-composite-keys.md) | module `composite_keys` | `hgl-rust` |
| [hgl-rust-deltas](hgl-rust-deltas.md) | module `deltas` | `hgl-rust` |
| [hgl-rust-direct-deltas](hgl-rust-direct-deltas.md) | module `direct_deltas` | `hgl-rust` |
| [hgl-rust-enums](hgl-rust-enums.md) | module `enums` | `hgl-rust` |
| [hgl-rust-execution-proof](hgl-rust-execution-proof.md) | module `execution_proof` | `hgl-rust` |
| [hgl-rust-families](hgl-rust-families.md) | module `families` | `hgl-rust` |
| [hgl-rust-finite-domains](hgl-rust-finite-domains.md) | module `finite_domains` | `hgl-rust` |
| [hgl-rust-generators](hgl-rust-generators.md) | module `generators` | `hgl-rust` |
| [hgl-rust-ir](hgl-rust-ir.md) | module `ir` | `hgl-semantics` |
| [hgl-rust-key-origins](hgl-rust-key-origins.md) | module `key_origins` | `hgl-rust` |
| [hgl-rust-keyed](hgl-rust-keyed.md) | module `keyed` | `hgl-rust` |
| [hgl-rust-layouts](hgl-rust-layouts.md) | module `layouts` | `hgl-rust` |
| [hgl-rust-mutation-bounds](hgl-rust-mutation-bounds.md) | module `mutation_bounds` | `hgl-rust` |
| [hgl-rust-observed](hgl-rust-observed.md) | module `observed` | `hgl-rust` |
| [hgl-rust-preparation](hgl-rust-preparation.md) | module `preparation` | `hgl-rust` |
| [hgl-rust-prepared-values](hgl-rust-prepared-values.md) | module `prepared_values` | `hgl-rust` |
| [hgl-rust-pure-bounds](hgl-rust-pure-bounds.md) | module `pure_bounds` | `hgl-rust` |
| [hgl-rust-recursive-convert](hgl-rust-recursive-convert.md) | module `recursive_convert` | `hgl-rust` |
| [hgl-rust-scalars](hgl-rust-scalars.md) | module `scalars` | `hgl-rust` |
| [hgl-rust-source-slots](hgl-rust-source-slots.md) | module `source_slots` | `hgl-rust` |
| [hgl-rust-structs](hgl-rust-structs.md) | module `structs` | `hgl-rust` |
| [hgl-rust-type-data](hgl-rust-type-data.md) | module `type_data` | `hgl-rust` |
| [hgl-rust-value-convert](hgl-rust-value-convert.md) | module `value_convert` | `hgl-rust` |
| [hgl-rust-values](hgl-rust-values.md) | module `values` | `hgl-rust` |
| [hgl-rust-windows](hgl-rust-windows.md) | module `windows` | `hgl-rust` |
| [hgl-scalar-copy](hgl-scalar-copy.md) | module `scalar_copy` | `hgl-store` |
| [hgl-scalar-keys](hgl-scalar-keys.md) | module `scalar_keys` | `hgl-semantics` |
| [hgl-shape-obligations](hgl-shape-obligations.md) | module `shape_obligations` | `hgl-semantics` |
| [hgl-shapes](hgl-shapes.md) | module `shapes` | `hgl-store` |
| [hgl-source-check](hgl-source-check.md) | module `source_check` | `hgl-semantics` |
| [hgl-static-values](hgl-static-values.md) | module `static_values` | `hgl-semantics` |
| [hgl-std-native](hgl-std-native.md) | module `std_native` | `hgl-stdlib` |
| [hgl-store-build](hgl-store-build.md) | module `store_build` | `hgl-store` |
| [hgl-struct-check](hgl-struct-check.md) | module `struct_check` | `hgl-semantics` |
| [hgl-struct-declarations](hgl-struct-declarations.md) | module `struct_declarations` | `hgl-semantics` |
| [hgl-struct-names](hgl-struct-names.md) | module `struct_names` | `hgl-semantics` |
| [hgl-test-annotations](hgl-test-annotations.md) | module `test_annotations` | `hgl-compiler` |
| [hgl-test-runner](hgl-test-runner.md) | module `test_runner` | `hgl-compiler` |
| [hgl-test-units](hgl-test-units.md) | module `test_units` | `hgl-compiler` |
| [hgl-time-context](hgl-time-context.md) | module `time_context` | `hgl-source` |
| [hgl-time-values](hgl-time-values.md) | module `time_values` | `hgl-types` |
| [hgl-type-shape](hgl-type-shape.md) | module `type_shape` | `hgl-source` |
| [hgl-type-sizes](hgl-type-sizes.md) | module `type_sizes` | `hgl-semantics` |
| [hgl-type-syntax](hgl-type-syntax.md) | module `type_syntax` | `hgl-source` |
| [hgl-value-access](hgl-value-access.md) | module `value_access` | `hgl-semantics` |
| [hgl-value-bind](hgl-value-bind.md) | module `value_bind` | `hgl-semantics` |
| [hgl-value-check](hgl-value-check.md) | module `value_check` | `hgl-semantics` |
| [hgl-value-constant](hgl-value-constant.md) | module `value_constant` | `hgl-semantics` |
| [hgl-value-eval](hgl-value-eval.md) | module `value_eval` | `hgl-semantics` |
| [hgl-value-lists](hgl-value-lists.md) | module `value_lists` | `hgl-store` |
| [hgl-value-operations](hgl-value-operations.md) | module `value_operations` | `hgl-semantics` |
| [hgl-value-types](hgl-value-types.md) | module `value_types` | `hgl-semantics` |
| [hgl-window-types](hgl-window-types.md) | module `window_types` | `hgl-types` |
| [hgl-wiring-locals](hgl-wiring-locals.md) | module `wiring_locals` | `hgl-semantics` |

## What a card holds

1. **Purpose** — one paragraph.
2. **May use** — the crates it may depend on. Nothing else.
3. **Surface** — every public item, as Rust signatures. An agent adds no
   public item that is not here; if it needs one, the card is wrong and says
   so in its report.
4. **Rules** — the specification rules the crate must satisfy, by number.
   Every rule gets at least one test that names it.
5. **Speed** — what [0009](../explorations/0009-designing-for-speed.md)
   means for this crate.
6. **Budget** — lines of code in `src/`, not counting blank lines, comments
   or doc comments. Held by `cargo xtask ci`.
7. **Done when** — the cases and benchmarks that must pass.
8. **Mutants** — named wrong implementations. Before reporting, the builder
   applies each one, shows a test failing, and reverts. A test that is named
   and green is not evidence it checks anything: in the first crate built
   this way, four wrong implementations passed every test until a reviewer
   tried them.

The reviewer works on a copy, tries the card's mutants, and **invents its
own**. In the second crate the builder killed all eight the card listed; the
reviewer then found twelve more that passed every test, one of which deleted
a whole column of the layout. That column turned out not to exist in the
reference either: a mutant that survives is sometimes the design telling us
something, and the question goes to hgraph's C++ before it goes to a test. A
reviewer's surviving mutants are added to the card, so the list grows with
what has actually gone wrong.

Mutants are tried **on a copy** of the repository, never in the shared tree:
other agents build against it, and a copy taken while a mutant is applied
carries the mutant with it. After restoring a file, `touch` it: a restore
that keeps the old modification time lets cargo reuse the mutant's build, and
the next "clean" run is not clean.

## The first slice (P1)

Built in this order; each needs the one before it to compile.

| # | Card | Budget | What it settles |
|---|---|---|---|
| 1 | [hgl-types](hgl-types.md) | 250 | Time, scalar and time-series types, the node type |
| 2 | [hgl-store](hgl-store.md) | 500 | The data layout: where values live and how a tick travels |
| 3 | [hgl-kernel](hgl-kernel.md) | 700 | **The node interface**, the schedule, the cycle, the engine |
| 4 | [hgl-describe](hgl-describe.md) | 450 | The graph description, the hand builder, instantiation |
| 5 | [hgl-proto-nodes](hgl-proto-nodes.md) | 250 | The nodes the cases and benchmarks need, written as an emitter would |
| 6 | [hgl-testkit harness](hgl-testkit-harness.md) | 300 | `run`, and the twins, made real |

2,450 lines for a runtime that moves one kind of value. The two to read
first are **hgl-kernel**, for the interface a node author sees, and
**hgl-store**, for the layout.

## Three things P1 does differently from the outline

- **No `unsafe` yet.** [Decision 0003](../decisions/0003-unsafe-confined-to-the-store.md)
  allows it in the store. P1 does not use it: for `Copy` scalars a node reads
  its inputs by value and then writes its output, so no two borrows overlap,
  and handles that are indices into plain vectors survive reallocation. It is
  introduced only if a benchmark misses its 5%
  ([0009](../explorations/0009-designing-for-speed.md), open question 1).
- **A node can already reschedule itself.** The outline put the node
  scheduler in P2, but every benchmark's source reschedules itself each
  cycle. P1 has the smallest form — one pending request, no tags, no cancel —
  and P2 completes it.
- **Nodes are boxed.** One allocation per node, at instantiation. The
  per-implementation slabs of 0009 matter when graphs are instantiated by the
  hundred thousand, which is P4, and they do not change what a node author
  writes.

## Dynamic slice

The [accepted traces](https://github.com/hhenson/hgraph_spec_audit/blob/main/archive/hgl/runtime/validation.md) drive one combined
TSD/REF/nested-graph slice. Scalar storage stays in `hgl-store`;
[hgl-bindings](hgl-bindings.md) owns endpoint identity and graph scopes,
[hgl-nested](hgl-nested.md) owns child instances, and
[hgl-deadlines](hgl-deadlines.md) shares a bounded schedule heap between the
kernel and child manager. Existing crate budgets stay unchanged.

## Fixed collection slice

[hgl-endpoints](hgl-endpoints.md) separates recursive shapes and slot storage
from binding policy. Fixed children use dense, stable slots.
[hgl-fixed-bench](hgl-fixed-bench.md) pairs three native collection scenarios
with C++; the 44 accepted semantic traces remain the correctness oracle.

## Recursive description infrastructure

[hgl-plan](hgl-plan.md) holds plain templates and structural checks;
[hgl-describe](hgl-describe.md) resolves them into scoped runtime instances.
This establishes the target for the later Rust compiler review.

The [compiler tooling spike](compiler-tools-spike.md) is an isolated comparison,
not a production compiler crate.

## Standard-library binding probe

[hgl-native](hgl-native.md) owns borrowed view/value helpers;
[hgl-stdlib](hgl-stdlib.md) tests the node authoring recipe before compiler emission.

## Compiled standard-library evaluation

[hgl-source](hgl-source.md) parses bodies; [hgl-library](hgl-library.md) indexes
modules; [hgl-program](hgl-program.md) checks and emits graphs and tests.
[hgl-columns](hgl-columns.md) owns typed scalar storage;
[hgl-calendar](hgl-calendar.md) parses/projects calendar values;
[hgl-std-native](hgl-std-native.md) supplies the selected Rust value functions.
The dense recorder remains in [hgl-testkit](hgl-testkit-harness.md).
