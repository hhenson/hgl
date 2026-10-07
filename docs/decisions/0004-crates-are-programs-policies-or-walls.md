# 0004 — A crate is a program, a compiler policy, or a wall between layers; everything else is a module

Status: accepted
Date: 2026-10-06
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
use" list and its line budget, and `cargo xtask ci` holds both:

```toml
[package.metadata.hgl]
line-budget = 5700          # the crate, as before

[package.metadata.hgl.modules]
bindings = { budget = 1100, uses = ["endpoints"] }   # src/bindings.rs plus src/bindings/**
```

`uses` is the card's "may use" list as sibling modules; the crate root is
always reachable. `xtask` grew by the module gate, so its budget is 580 (the gate selection and per-gate cargo arguments that let CI run and shard the test stages as parallel jobs are held there too).

The workspace is the fourteen packages listed in the exploration. The
budgets of the merged crates became module budgets, unchanged, and each
crate's budget is the sum of what it absorbed. A module budget is raised the
way a crate budget was: by a decision, never to pass a build.

Decision 0003 is unchanged in substance: `unsafe` stays confined to
`hgl-store`. Once the store holds any, the crate's own lint drops from
`forbid` to `deny`, every module that must stay safe declares
`#![forbid(unsafe_code)]` as an inner attribute, and the audited unit is
that one module rather than the crate.

## Reason

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
