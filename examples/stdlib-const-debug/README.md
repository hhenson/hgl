# Standard-library const → debug_print

```sh
python3 tools/shared_artifacts.py
python3 examples/stdlib-const-debug/run.py --out-dir /tmp/hgl-stdlib-const-debug
```

Prints `answer: 42` once. `main.hgl` is materialized from hgraph_spec. It imports
both operators from the pinned hgraph_std; their definitions and implementations
are read unchanged. The compiler selects the i64 implementations, resolves the
fixed defaults and emits source, sink and graph construction. `rust.hgl` and
`native.rs` supply only integer-to-text and line printing. `main.rs` launches the
existing Rust engine.

```sh
cargo run -p hgl-compiler --bin hglc -- check examples/stdlib-const-debug/main.hgl --library external/hgraph_std/hgl/hgraph --part examples/stdlib-const-debug/rust.hgl
cargo run -p hgl-compiler --bin hglc -- emit-rust examples/stdlib-const-debug/main.hgl --library external/hgraph_std/hgl/hgraph --part examples/stdlib-const-debug/rust.hgl --out /tmp/stdlib-graph.rs
```

The library path is an explicit source dependency. Native parts remain HGL code;
the emitted `Native` trait makes the selected Rust signatures concrete. No
operator implementation is selected by a built-in name table.

This is a closed graph slice: fixed arguments are captured during wiring.
Temporal endpoints admit i64/bool; strings are fixed labels and native formatting
values. Arbitrary collection endpoints, generators, scheduler handlers, runtime
alarm rearming and library documentation export are not implemented. A selected
unsupported construct is rejected. Unused library bodies are indexed and checked
only when selected. The old self-contained bootstrap remains a regression case.

Tests compile generated Rust and check values, publication times, printed lines,
delays, sampling, missing/equal ticks, independent runs, changed HGL bodies and a
renamed operator. The shared cases and Python/C++ observations live in
[hgraph_spec](https://github.com/hhenson/hgraph_spec/tree/codex/stdlib-const-debug/compiler/stdlib_graph)
and [hgraph_spec_audit](https://github.com/hhenson/hgraph_spec_audit/tree/codex/stdlib-const-debug/compiler/stdlib_graph).
