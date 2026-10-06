# Decisions

Short records of what was settled. A decision is not a design document: it
states the choice, the reason, and what was given up, and links to the
exploration that holds the detail.

Name a record `NNNN-short-slug.md`. Numbers here are independent of the
exploration numbers and of the decision records in `hgraph`.

| # | Decision | Status | Exploration |
|---|---|---|---|
| 0001 | [The new compiler back end, runtime and generated code are Rust](0001-rust-end-to-end.md) | accepted | [0001](../explorations/0001-implementation-language-survey.md) |
| 0002 | [Performance is judged against the C++ runtime, within 5%](0002-performance-parity-with-cpp.md) | accepted | [0009](../explorations/0009-designing-for-speed.md) |
| 0003 | [`unsafe` is allowed in the store crate, and nowhere else](0003-unsafe-confined-to-the-store.md) | accepted | [0009](../explorations/0009-designing-for-speed.md) |
| 0004 | [A crate is a program, a compiler policy, or a wall between layers; everything else is a module](0004-crates-are-programs-policies-or-walls.md) | accepted | [0011](../explorations/0011-crate-boundaries.md) |

## Template

```markdown
# NNNN — The decision, stated as a sentence

Status: accepted
Date: YYYY-MM-DD
Exploration: ../explorations/NNNN-slug.md

## Decision

What was chosen, precisely enough that someone could tell whether a piece of
HGL complies with it.

## Reason

Why this and not the alternatives. One paragraph.

## Given up

What this rules out or makes harder. Every real decision has something here.

## Upstream

Link to the hgraph RFC or decision record, once there is one.
```

A decision that is later reversed is not deleted: mark it
`superseded by <link>` and write the new one.
