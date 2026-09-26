# hgl

A private experiment: a new compiler back end and a compact runtime for the
**hgraph language** (HGL), written in Rust — and, as much as the code, a
method for describing a system so that AI agents produce code that stays
clean, compact and understandable.

**Status:** experimental and possibly throw-away. Nothing here is a
commitment, a specification, or a release. Do not share.

## Relationship to hgraph

The language is specified in the [`hgraph`](https://github.com/hhenson/hgraph)
repository under `language/`, and this repository accepts it as it stands.

- **The specification is upstream.** HGL's syntax and semantics are whatever
  `hgraph/language/docs` says. Implementation choices — how programs are
  built and executed — are free to differ here.
- **Extensions land upstream first.** A needed language extension is proposed
  in hgraph, made to work there, and only then used here.
- **The hgraph runtime is the reference.** It mostly does what is needed; its
  behaviour is the oracle the new runtime is checked against, tick for tick.
  The emitter and the native bindings here are new.

## What is here

| Path | Contents |
|---|---|
| [`docs/decisions/`](docs/decisions/README.md) | What is settled, and why |
| [`docs/explorations/`](docs/explorations/README.md) | What is being worked out, one file per topic |
| [`docs/guides/rust-practices.md`](docs/guides/rust-practices.md) | The Rust setup, how to read Rust from C++, how to review a diff |
| [`docs/open-questions.md`](docs/open-questions.md) | Questions nobody has picked up yet |
| `Cargo.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` | The workspace and its gates |
| `xtask/` | Repository automation |
| `CLAUDE.md` | The rules for agents working here |

The Rust runtime supports scalar values, fixed collections, dictionaries,
references and nested graphs through its API; see [coverage and validation](docs/runtime-implementation.md).
The first Rust compiler slice generates complete i64 const/debug nodes and their
graph construction; [run the example](examples/const-debug/README.md). General
language coverage and recursive compiler lowering remain pending.

## Building

```sh
cargo xtask ci    # format, clippy, tests, docs, dependency policy
```

That command is the definition of done. CI runs it on Linux and Windows;
macOS is run locally.

## Baseline

When a doc here says "today", it means the language as implemented in
`hgraph/language/` at the time of writing. The places to check that claim:

- `language/docs/design/` — language model, control flow, iteration, operators,
  modules, native interface, roadmap, and the numbered decision records
- `language/docs/developer-guide/syntax-and-semantics.md` — the closest thing to
  a reference, including the EBNF grammar
- `language/examples/` and `language/stdlib/` — what actually parses and runs
