# hgl

A private experiment: a new compiler back end and a compact runtime for the
**hgraph language** (HGL), written in Rust — and, as much as the code, a
method for describing a system so that AI agents produce code that stays
clean, compact and understandable.

**Status:** experimental and possibly throw-away. Nothing here is a
commitment, a specification, or a release. Do not share.

## Relationship to hgraph

The language and runtime contracts live in
[hgraph_spec](https://github.com/hhenson/hgraph_spec), with portable HGL in
[hgraph_std](https://github.com/hhenson/hgraph_std). This repository implements
them in Rust; hgraph implements the compiler and runtime in C++.

Language changes belong in the shared specification first. The
[audit project](https://github.com/hhenson/hgraph_spec_audit) compares independently
reasoned expectations with Python 0.5.x and C++ 0.8.x, retaining accepted
variations and owner rulings. Native implementations stay with their runtime.

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
language coverage and recursive compiler lowering remain pending. The
[type and wiring review](docs/compiler/wiring-review.md) maps the updated runtime
contract to the Rust implementation and its next acceptance cases.

## Building

```sh
python3 tools/shared_artifacts.py
cargo xtask ci    # format, clippy, tests, docs, dependency policy
```

That command is the definition of done. CI runs it on Linux and Windows;
macOS is run locally.

## Shared language sources

Run `python3 tools/shared_artifacts.py` before compiler, documentation or
contract-test work. See [shared source setup](docs/shared-sources.md).

## Run the current standard-library graph

```sh
python3 tools/shared_artifacts.py
python3 examples/stdlib-const-debug/run.py
```

Prints `answer: 42` once. The HGL graph imports `const` and `debug_print` from
the pinned standard library and runs their compiled HGL bodies on the Rust
engine. See [the example](examples/stdlib-const-debug/README.md) for the compiler
commands, native bindings and current supported scope.

## Run the standard-library tests

```sh
python3 tools/test_hgl.py --stdlib
```

Compiles and runs the unchanged library's 45 tests and 84 eval assertions on
Rust. These also run in `cargo xtask ci`. See [eval execution](docs/compiler/eval.md)
for custom modules, native bindings and the supported compiler slice.
