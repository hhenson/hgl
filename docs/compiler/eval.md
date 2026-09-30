# HGL tests on the Rust engine

```sh
python3 tools/shared_artifacts.py
python3 tools/test_hgl.py --stdlib
```

Runs the pinned hgraph_std: **83 named tests, 132 evaluations**,
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

The compiler wires the standard library's `replay` sources, the target, and its
`record` sink, resolving and compiling their ordinary HGL bodies. Replay's HGL
body controls its cursor, scheduling and publication; record's HGL body begins
the recording and captures each admitted delta. Native capabilities provide
typed buffer access and owned storage. Each evaluation starts fresh; `_` means
no tick, including trailing silent cells. Expected values never control the
run's horizon.

Replay reads its configured sequence with `replay_input[index]`. An in-range
absent slot returns `null`; an out-of-range index fails. The HGL body checks an
immutable local before using its payload:

```hgl
let item = replay_input[current]
if item != null { return item }
```

Zero, `false` and empty text are present values. `len(replay_input)` counts
absent slots too. Capability actions use receiver-first function calls,
including `schedule(alarm, delay)`, `begin(capture)` and
`append(capture, last_modified(ts), delta_value(ts))`. Clock observations use
read-only property access: `clock.evaluation_time` and
`clock.next_cycle_evaluation_time`. The specified `clock.now` observation is
not yet implemented by this compiler slice.

The shared pass-through is one generic runtime compute:

```hgl
fn pass_through<T>(value: T) -> T {
    when { return delta_value(value) }
}
```

`delta_value(value)` extracts the current delta. `delta<T>(...)` is a distinct
constructor form; `delta(value)` is not an accessor alias. This compiler admits
delta extraction for bool, i64, f64, str, date, time, datetime and duration when
the particular endpoint is proven valid and modified. The 38 scalar replay/pass-through HGL tests
exercise 48 evaluations covering those types, silence, equal ticks, independent
inputs, fresh recordings and outputless runs.

The [compiler card](../cards/hgl-program.md) lists supported forms and remaining
limits. Direct scalar eval assertions work; timed sequences, structural delta
literals, general test expressions and harness locals do not yet. The runtime's
recursive collection APIs are broader than compiler lowering in this slice.

Shared expectations and Python/C++ comparison evidence belong to the
[delta-evaluation audit](https://github.com/hhenson/hgraph_spec_audit/tree/codex/replay-sequence-indexing/runtime/validation/delta_eval).
The specification defines HGL concepts and rules; implementation observations
and differences are recorded separately in that audit.
