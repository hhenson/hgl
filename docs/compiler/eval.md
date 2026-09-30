# HGL tests on the Rust engine

```sh
python3 tools/shared_artifacts.py
python3 tools/test_hgl.py --stdlib
```

Runs the pinned, unchanged hgraph_std: **45 named tests, 84 evaluations**,
including tests embedded beside native interfaces. `cargo xtask ci` also
compiles and runs the complete suite in its debug and release test gates.

For a module of your own:

```sh
python3 tools/test_hgl.py path/to/tests.hgl \
  --library external/hgraph_std/hgl/hgraph \
  --part native/stdlib/rust.hgl --native-rust native/stdlib/native.rs
```

`hglc emit-tests FILE [--part FILE] [--library DIR] --out tests.rs` emits the
checked test executable. The Python runner packages it with the selected Rust
provider and builds/runs it. `--build-dir DIR` retains that crate for inspection.
A mismatch names the test and first differing cycle, then exits unsuccessfully.

The compiler resolves the actual operator declarations and HGL implementations.
It adds replay and recorder nodes around each graph and runs the ordinary Rust
simulation engine. Each evaluation starts fresh; `_` means no tick, including
trailing silent cells. Expected values never control the run's horizon.

The [compiler card](../cards/hgl-program.md) lists supported forms and remaining
limits. Direct scalar eval assertions work; timed sequences, structural delta
literals, general test expressions and harness locals do not yet. The runtime's
recursive collection APIs are broader than compiler lowering in this slice.

Shared expectations and Python/C++ comparison evidence belong to
[hgraph_spec_audit](https://github.com/hhenson/hgraph_spec_audit/tree/codex/stdlib-eval/compiler/stdlib_eval).
The C++ comparison uses its native-provider branch because current main does
not yet implement the diagnostic native interfaces required by this library.
