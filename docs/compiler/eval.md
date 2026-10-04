# HGL tests on the Rust engine

```sh
python3 tools/shared_artifacts.py
python3 tools/test_hgl.py --stdlib
```

Runs every named test and evaluation in the pinned hgraph_std, including tests
embedded beside native interfaces. `cargo xtask ci` also compiles and runs the
complete suite in its debug and release test gates.

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
body controls timed publication; record's HGL body retains each admitted delta
in ordinary run-owned storage. Each evaluation starts fresh; `_` means
no tick, including trailing silent cells. Expected values never control the
run's horizon.

Replay receives ordinary `list<TimedValue<T>>` data. Only present dense input
cells become timed entries; each entry retains its exact `delta<T>` publication
and absolute target. The HGL generator yields those entries in order. Record
appends ordinary timed publications to a prepared typed global-state list.
Zero, `false` and empty text are present values. Read-only clock properties
provide `clock.evaluation_time` and `clock.next_cycle_evaluation_time`; the
specified `clock.now` observation is not yet implemented by this compiler slice.

The shared pass-through is one generic runtime compute:

```hgl
fn pass_through<T>(value: T) -> T {
    when { return delta_value(value) }
}
```

`delta_value(value)` extracts the current delta when its endpoint is proven
valid and modified. `delta<T>(...)` is a distinct constructor form;
`delta(value)` is not an accessor alias. The finite publication profile admits
the eight scalar leaves, bool/i64 sets, fixed lists, positional tuples, concrete
nonrecursive nominal structs and i64-keyed maps recursively. Shared tests cover
sparse child omissions, equal repeated ticks, removals, silence and fresh runs.
Atomic lists, tuples and concrete structs publish complete ordinary values; an
empty list is a present snapshot. Defaults are reconstructed for each snapshot,
and retained recordings do not alias later source or sibling mutations.
For each supported non-composite type S, `atomic<S>` is the same type as S;
composite boundaries remain significant in matching and generic arguments.

Non-null fixed scalar struct field defaults are admitted, including in concrete
generic specializations. Complete ordinary constructors fill omitted defaults
after retaining supplied fields; sparse delta constructors never apply defaults.
General constant-helper defaults, optional fields and inheritance remain
unsupported by this increment. These are compiler limits, not changes to HGL.

The [compiler card](../cards/hgl-program.md) lists supported forms and remaining
limits. Direct eval assertions, deterministic ordinary assertions and ordinary
test bindings work; timed input syntax remains unsupported. References, growing temporal
lists, windows, atomic sets/maps and recursive or optional atomic payloads remain
outside the finite eval profile. The compiler currently supports eight scalar
leaves; other temporal scalars and enums remain unimplemented.

Shared expectations and Python/C++ comparison evidence belong to the
[delta-evaluation audit](https://github.com/hhenson/hgraph_spec_audit/tree/main/runtime/validation/delta_eval).
The specification defines HGL concepts and rules; implementation observations
and differences are recorded separately in that audit.
