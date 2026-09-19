# hgl

A private scratch space for experimenting with the design and evolution of the
**hgraph language** (HGL).

**Status:** experimental and probably throw-away. Nothing here is a commitment,
a specification, or a release. Do not share.

## What this is for

The working language — compiler, standard library, examples and its design
corpus — lives in the [`hgraph`](https://github.com/hhenson/hgraph) repository
under `language/`. That tree is held to a standard: docs are authoritative,
changes land green, structural proposals go through an RFC.

This repository is the opposite. It is where ideas get written down quickly,
argued with, rewritten and thrown away, without the cost of keeping a compiler
and a test suite in step. An idea that survives graduates back to `hgraph` as
an RFC or a design decision record; one that does not is left here as a record
of why.

## What is here

Docs only, for now. No compiler, no build, no code.

| Path | Contents |
|---|---|
| [`docs/README.md`](docs/README.md) | Index and the conventions the docs follow |
| [`docs/explorations/`](docs/explorations/README.md) | One file per idea being explored |
| [`docs/decisions/`](docs/decisions/README.md) | Short records of what was settled, and why |
| [`docs/open-questions.md`](docs/open-questions.md) | Running list of questions nobody has answered yet |

## Why markdown

Everything is plain markdown because it is the quickest format to read and to
edit, for a person and for an AI session alike. No build step, no rendered
site, no cross-reference machinery: a doc is a file, a link is a relative path.

## Baseline

When a doc here says "today", it means the language as implemented in
`hgraph/language/` at the time of writing. The places to check that claim:

- `language/docs/design/` — language model, control flow, iteration, operators,
  modules, native interface, roadmap, and the numbered decision records
- `language/docs/developer-guide/syntax-and-semantics.md` — the closest thing to
  a reference
- `language/examples/` and `language/stdlib/` — what actually parses and runs
