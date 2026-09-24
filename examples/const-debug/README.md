# First compiled graph

```sh
python3 examples/const-debug/run.py --out-dir /tmp/hgl-const-debug
```

Prints `42` once. The generated crate remains in the requested directory.
`main.hgl` defines the source, sink and graph. `rust.hgl` selects the native
print helper; `native.rs` implements it. `main.rs` is only an engine launcher:
node bodies, their metadata, native interface and graph construction are emitted.

The source is copied from hgraph's tested `language/examples/const-debug.hgl`;
the Rust part and provider belong here. hgraph supplies its own C++ implementation.
The bootstrap is i64-only and prints no timestamp or label.

```sh
cargo run -p hgl-compiler --bin hglc -- check examples/const-debug/main.hgl --part examples/const-debug/rust.hgl
cargo run -p hgl-compiler --bin hglc -- emit-rust examples/const-debug/main.hgl --part examples/const-debug/rust.hgl --out /tmp/const-debug.rs
```

Checking a bare native declaration is allowed. Emission requires its selected
implementation. Unsupported syntax or types fail before output is written.
See [expected traces](../../docs/compiler/bootstrap/README.md) and the
[compiler contract](../../docs/cards/hgl-compiler.md) for the exact subset.
