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
`delta(value)` is not an accessor alias. The current publication matrix covers:

| Publication | Admitted profile |
| --- | --- |
| Scalars | `bool`, `i64`, `f64`, `str`, `date`, `time`, `datetime`, `duration`, `civil_datetime`, `timezone`, `zoned_time`, `zoned_datetime`, and nominal enums. |
| Structural values | Sparse `set<K>`, `map<K,S>`, fixed lists, growing lists, positional tuples, and concrete required-field structs with admitted children. Growing lists append contiguous positions and remove only a contiguous tail. |
| Complete atomic values | Ordinary lists, tuples, concrete structs, sets and maps; optional fields; finite recursive structs; and nonrecursive abstract families retaining the exact concrete member tag. |
| Rolling windows | `rolling<V, Max, Min>` with an admitted ordinary payload and exact tick-count or duration bounds. Its delta is the arriving `V`; readiness is separate from validity. |
| Collection keys | Scalar and enum keys, plus finite tuples and concrete structs, including optional-field presence. Recursive, family, collection, reference and native opaque key components are excluded. NaN keys are rejected; signed zeros compare equal. |

These are the profiles in the pinned specification, not arbitrary combinations
of recursive or container types. Recursive edges must be optional direct atomic
fields with null defaults, finite specialization graphs and finite value trees.
Complete construction applies declared defaults to omitted fields; optional
fields with null defaults remain unset, and explicit null is allowed only for
optional fields. This adds no optional-field read or sparse clearing operation.
Sparse deltas never fill defaults.
For each admitted scalar S, `atomic<S>` is the same type as S; composite boundaries,
nominal arguments, field presence and window bounds retain their exact identity.
Empty atomic containers and all-unset structs are present snapshots. Recordings
own their values independently of later source or sibling mutations.

Preparation fixes types before graph start and materializes provider-dependent
values in source order. A separate finite execution proof selects prepared storage
for bounded schedules, membership changes and owning payload sizes. The prepared
fixtures check allocation-free `graph.evaluate` calls and retained capture semantics.
This is not an allocation guarantee for every graph accepted by the compiler:
unproved plans use the existing generic adapter and its allocation behavior.
Neither native arbitrary key types nor unbounded owning growth is implied.

Direct eval assertions, outputless evals, ordinary test bindings, bound eval
captures and guarded assertions are supported. Captures retain a logical horizon
with sparse present entries; indexed payload use requires a presence proof.
Explicit timed input syntax remains unsupported. References are explicitly
excluded from this publication test matrix; existing compiler reference support
is separate. Signal coverage is observation-only, without payload recording.
The [compiler card](../cards/hgl-program.md) records additional limits, including
uninitialized locals and unsupported contextual construction paths.

All ordinary eval argument expressions finish in their written order before input traces are validated. A malformed publication sequence raises eval.input_delta_profile with the parameter and zero-based publication slot before graph startup. Ordinary list<delta<T>> bindings retain their exact publication element identity and evaluate once when supplied as a sequence.
