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

## Enforced ordinary value model boundary

The native-value integration separates hgl-value-model from source parsing under
reason 3. Before extraction the moved model has 1081 code lines and the remaining
source parser/descriptor layer has over 1100. The model depends on hgl-types and
the already-approved jiff library; source depends on the model and cannot be
reached from it. This prevents canonical value operations and execution-context
materialization from reading parser syntax or discovering native implementation
images. Checked IR and its traversal stay with this syntax-neutral model. Source spelling
and literal parsing remain in hgl-source. Numerical reporting targets are
informational and do not limit these modules.

## Enforced checked execution boundary

hgl-eval reexports Model checked IR and owns ordinary execution, with only hgl-value-model and
hgl-types dependencies. Both this layer and remaining semantic admission exceed
1000 lines. Source constructor/boxing/delta admission remains in semantics,
which reexports the lower IR and operation paths for existing consumers. Thus an
ordinary execution operation cannot consult source declarations or discovery as
an implicit fallback. Its aggregate reporting target is 2000; numerical targets remain informational. This is the
second reason-3 boundary; it adds no third-party dependency.

## Enforced Rust assembly boundary

The Rust backend separates checked value/layout/hook emission into
hgl-rust-codegen, below the hgl-rust program-assembly facade. The moved layer
has 7492 code lines; the remaining graph/generator assembly, capacity provisioning
and test adapters have 1504. This meets reason 3 on both sides. Cargo prevents
codegen from reaching assembly or provisioning: the facade depends on codegen,
and the reverse dependency would form a cycle. Module dependency lists remain enforced; aggregate and module line-count targets
are informational. The facade reporting target is 1700. Public facade reexports preserve consumers. No runtime or third-party
dependency is introduced.
