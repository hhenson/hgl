# Explorations

One file per idea. Name it `NNNN-short-slug.md`, numbered in the order it was
started; the number is only there to give a stable way to refer to it.

| # | Title | Status |
|---|---|---|
| 0001 | [Implementation language and tooling for the new runtime](0001-implementation-language-survey.md) | accepted |
| 0002 | [Profile of the reference implementation](0002-reference-implementation-profile.md) | sketch |
| 0003 | [What the evidence says about AI agents, languages and clean code](0003-ai-codegen-evidence.md) | sketch |
| 0004 | [Languages and tools for writing compilers, 2026](0004-compiler-tooling-survey.md) | sketch |
| 0005 | [The shape of the code](0005-code-shape.md) | sketch |
| 0006 | [What we need to do, and in what order](0006-build-sequence.md) | sketch |
| 0007 | [Where hgraph's own documents disagree](0007-hgraph-doc-conflicts.md) | sketch |
| 0008 | [Prototype runtime: outline](0008-prototype-outline.md) | sketch |
| 0009 | [Designing for speed](0009-designing-for-speed.md) | sketch |
| 0010 | [Rust compiler tooling comparison](0010-rust-compiler-tooling.md) | measured spike; recommendations proposed |
| 0011 | [Crate boundaries](0011-crate-boundaries.md) | measured; layout adopted by decision 0004 |

## Template

Copy this into a new file. Delete any heading that has nothing under it.

```markdown
# NNNN — Title

Status: sketch

## The idea

Two or three sentences. What changes for someone writing HGL?

## What it looks like

The proposed code first. If there is a way to write the same thing today,
show that beside it, labelled *today* and *proposed*.

## Why

The problem this solves, with a real case where today's language hurts.

## What it costs

What it breaks, what it complicates, what it makes ambiguous. Interactions
with the rest of the language: types, temporal semantics, control flow,
operators and overloads, modules, the native boundary.

## Alternatives

Other shapes considered, and why this one is preferred, if it is.

## Open questions

What would have to be answered before this could be decided.
```
