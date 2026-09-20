# Docs

## Index

- [Explorations](explorations/README.md) — ideas in progress, one file each
- [Decisions](decisions/README.md) — what was settled, and why
- [Open questions](open-questions.md) — unanswered, unowned
- [Rust practices](guides/rust-practices.md) — the setup, reading Rust from
  C++, reviewing a diff
- [Contract cards](cards/README.md) — the one page an agent is given to build
  one crate; the first slice's six are written and await review
- [Runtime specification](runtime_spec/overview.md) — what an HGraph runtime
  is, concept by concept; in progress

Reading order: [decision 0001](decisions/0001-rust-end-to-end.md) (Rust, end
to end) and the survey behind it (explorations 0001–0004); then
[0005 — the shape of the code](explorations/0005-code-shape.md) and
[0006 — the order of work](explorations/0006-build-sequence.md), both sketches.

## Conventions

Kept deliberately light. The point is speed; drop any rule that gets in the way.

**One topic per file, and keep it short.** A doc that needs a table of contents
is two docs. Prefer a new file over a long one.

**Start every doc with a status line**, so a reader knows how much weight to
give it without reading it:

```
Status: sketch | proposed | accepted | rejected | superseded by <link>
```

**Say whether syntax is real.** Every `hgl` code block is one of two things,
and the doc must make clear which:

- *today* — it parses and runs with the current compiler in `hgraph/language/`
- *proposed* — it does not exist yet; this is what it might look like

Mixing the two silently is the fastest way to make these docs useless.

**Show the example first.** A language idea is judged by what the code looks
like. Lead with the snippet, then explain it, then list what it breaks.

**Record the rejections.** A rejected idea with its reason is worth more than a
deleted one; it stops the same argument being had twice. Change the status,
add the reason, leave the file.

**Link with relative paths.** No site generator, no anchors that depend on one.

## Graduating an idea

When an exploration is settled enough to build:

1. Write the decision record here (`decisions/`).
2. Carry it to `hgraph` as an RFC or a `language/docs/design/decisions/` record,
   in that repository's format and to its standard.
3. Mark the exploration here `superseded by` the upstream record.

This repository never becomes the source of truth for anything that ships.
