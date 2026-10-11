# 0004 — A crate is a program, a compiler policy, or a wall between layers; everything else is a module

Status: accepted
Date: 2026-10-06; size policy revised 2026-10-11
Exploration: ../explorations/0011-crate-boundaries.md

## Decision

A new crate is added only when one of three things is true:

1. it is a separate program — a binary, or tooling that is never linked into
   the product;
2. it needs a different compiler policy — `unsafe` allowed, another lint
   table, Miri;
3. it is a layer the compiler must stop another layer from reaching, and
   both sides are over a thousand lines.

Everything else is a module in the crate that owns the idea. A contract card
describes a module as readily as a crate; the module keeps the card's "may
use" list, enforced by `cargo xtask ci`. Crate and module line counts are
informational. Existing numeric metadata remains an optional reporting target;
exceeding or omitting it does not fail CI:

```toml
[package.metadata.hgl]
line-budget = 5700          # optional informational crate target

[package.metadata.hgl.modules]
bindings = { budget = 1100, uses = ["endpoints"] }   # src/bindings.rs plus src/bindings/**
```

`uses` is the card's "may use" list as sibling modules; the crate root is
always reachable. The existing `budget` gate reports every crate and declared
module, with its target when present. It fails on forbidden sibling reach or
unreadable inputs, never on source size. A module without `budget` still has
its `uses` checked. Do not split code or add crates merely to reduce a count.
Lint, unsafe confinement, correctness tests and performance requirements are
unchanged; the three reasons for creating a crate above remain binding.

Decision 0003 is unchanged in substance: `unsafe` stays confined to
`hgl-store`. Once the store holds any, the crate's own lint drops from
`forbid` to `deny`, every module that must stay safe declares
`#![forbid(unsafe_code)]` as an inner attribute, and the audited unit is
that one module rather than the crate.

## Reason

The following records the original consolidation rationale, before line
counts became informational.

The crate count had become a side effect of two good rules — budgets are
never raised, cards are written per unit of work — rather than a design.
124 crates for 31,000 lines put a crate at every 250 lines; one layer of the
store was spread over four manifests and stitched back together with
re-exports; the compiler wrote internal crate paths into generated
programs. Nothing the split bought needed a crate: a module gives the same
privacy, the same `unsafe` confinement and the same testability, and a
budget gate counts a directory as easily as a crate. Compile time was not a
factor at this size — a cold build of the whole workspace took under a
second.

## Given up

- Cargo no longer enforces a card's "may use" list between modules of one
  crate. The `xtask` gate holds it instead, by inspecting `crate::` paths,
  which is weaker by one case: a re-export in `lib.rs`.
- A merged crate's `pub` items are visible to every module in it. The
  reviewer's "did the public surface grow" check now measures what leaves the
  crate, which is what it was meant to measure.
- A larger `hgl-store` to audit if `unsafe` ever lands, mitigated by the
  per-module `forbid` above.
- Historical documents name the old crates. Cards keep their names and say
  which module they describe; explorations are left as dated records.

## Upstream

None.

## Enforced ordinary metadata boundary

hgl-value-model owns syntax-neutral ordinary metadata, checked IR definitions
and contextual scalar materialization under reason 3. Source type-spelling and
literal parsers remain in hgl-source. The source crate and model each exceed
1000 lines under xtask's code-line count; the execution layer also exceeds 1000.
The model depends only on hgl-types and the existing jiff dependency, so it
cannot reach source AST, parsing or semantic name resolution. Facade reexports
preserve existing clients. The model ceiling is 1200; moved module ceilings are
split or transferred without increasing any crate or module ceiling.
