# Contract cards

Status: accepted for the first slice, 2026-09-19 — revised as each build finds what a card left out

A card is the one page an agent is given to build one crate. It is written by
us, not generated. With it the agent gets the rules of the
[runtime specification](../runtime_spec/overview.md) the card names, and the
failing cases and benchmarks the card names — not the whole repository.

This is the method under test
([0006](../explorations/0006-build-sequence.md), "How each crate gets
built"): card → agent → a reviewer with fresh context who sees only the diff
and the card → the owner. When something comes out wrong, the card, a lint, a
budget or a test is fixed, and the work is regenerated.

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

The [accepted traces](../runtime_spec/validation.md) drive one combined
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
