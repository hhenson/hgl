# Nodes and Rust bindings

Status: runtime specimens; [native interface migration](native-interfaces.md) in progress.

Author library behaviour in HGL. Keep its guards, state, scheduling and writes
there; port native value/view helpers to Rust. Existing HGL is the source of
truth. Do not introduce a `rust {}` source escape to port C++ bodies.

The current upstream body, specialized to `i64`, is:

```hgl
impl fn bit_and(lhs: i64, rhs: i64) -> i64 {
    when { return native::bit_and(lhs, rhs) }
}
```

Its Rust node holds `In<i64>` handles and an `Out<i64>`, implements `Node`, and
calls `hgl_native::bit_and_i64` directly. `Buildable` constructs handles once.
`NodeType` declares both inputs active and required valid. The helper receives
two values; it cannot schedule, publish or retain an input view.

## Binding contract

- **NAT-1:** Select by canonical module/declaration identity, full signature and
  role. `native const fn hgraph.native.bit_and(i64,i64)->i64` is a value helper;
  `hgraph.operators.bit_and` is a temporal operator. Neither substitutes for
  the other. Resolve candidates in the checker, retain the selection in IR.
- **NAT-2:** Generate the implementation interface from the HGL contract. C++
  uses a checked `bind<T>()` adapter; Rust implements a generated trait. Keep
  implementations in native source and dependencies in its normal library build.
  Select the provider once, without a separately authored function-symbol map.
  Missing, ambiguous or incompatible providers are compile errors.
- **NAT-3:** Value helpers receive admitted payloads. Input-view helpers borrow
  the current input, including its local binding state. The borrow ends with
  the call; no copying collections, retaining views or resolving output names
  per tick. `signal` queries must also work before validity.
- **NAT-4:** A temporal implementation owns activation, guards, state, lifecycle
  and writes. A helper call adds no node and emits no tick on its own. Keep
  equal ordinary ticks; suppress equal REF designations under TS-16.
- **NAT-5:** HGL `state` is semantic history, requiring checkpoint restoration;
  `cache` must be reconstructible. A plain Rust field proves fresh-run behaviour
  only. Fallible helpers will return a typed `Result`, translated at the node
  boundary; catching panics is not the native error contract.
- **NAT-6:** Validate a shaped handle at construction. Metadata helpers inspect
  the input view for peered and assembled TSL/TSB, TSD and REF. Do not infer
  input time or validity by inspecting only its peer. Preserve the accepted
  removal, invalidation, rebinding and child-scope rules.
- **NAT-7:** Check capability requirements for HGL and native functions alike.
  Calls silently add the callee's requirements to the caller, transitively and
  without duplicates. `inject out` refers to the declared temporal result. Value functions may use
  admitted context services without acquiring a node. Selected HGL implementation parts declare
  target requests and lifecycle hooks; the shared declaration owns the signature. See the
  [capability contract](native-interfaces.md#implementation-parts-and-injectables).

`ref<ref<T>>` in source is an error. Substituting `T = ref<U>` into `ref<T>`
normalizes to `ref<U>` before target mapping. No nested REF runtime endpoint.

`native fn` follows ordinary temporal typing. Only `native const fn` is a
value helper; parameter-level `const` does not change execution role. Borrowed
input access must be explicit in the shared contract.

## Concrete native projections

| HGL helper | Rust call | Projection |
|---|---|---|
| `hgraph.native.bit_and(i64,i64)->i64` | `hgl_native::bit_and_i64(lhs, rhs)` | Current values; no borrowed result |
| `hgraph.native.valid(signal)->bool` | `InputView::valid` | Immutable live input, evaluation only |
| `hgraph.native.all_valid(signal)->bool` | `InputView::all_valid` | Immediate-child validity, not recursive leaf completeness |
| `hgraph.native.modified(signal)->bool` | `InputView::modified` | Current cycle and input-local state |
| `hgraph.native.last_modified(signal)->datetime` | `InputView::last_modified` | Engine time available; datetime payload mapping pending |

A generated call creates `InputView::new(ctx.store(), input, ctx.evaluation_time())`
for that call only. An assembled input need not have a peer output. Fixed
projections are resolved during construction; dynamic member views are borrowed
from current membership, never cached past removal or child teardown.

The future mapping catalogue must test wrong signatures, forbidden phases,
borrowed-result escape, ambiguous candidates and missing target capabilities.
These are compiler acceptance scenarios, not checks implemented by this probe.
The ordinary Rust compiler checks the concrete helper calls already present.

## What the review found

| Layer | Present | Work needed |
|---|---|---|
| Node recipe | `Node`, `Ctx`, `Buildable`; construction-time port lookup | Emit the recipe from checked bodies; ordered handlers and early returns |
| Wiring | Recursive descriptions, peered/assembled ports, scoped children | Preserve resolved contracts and specializations into descriptions |
| Selection | Registry of concrete `NodeType`s | Canonical identities, overloads, constraints, capabilities; registry is not the HGL resolver |
| Native helpers | This probe's value calls and borrowed metadata | Validated target catalogue and module import integration |
| State/lifecycle | Fields, start/eval/stop, scheduling, child teardown | Recordable state and scheduler recovery; owned native resource/error contracts |
| Types | bool/i64/f64, fixed TSL/TSB, i64-keyed TSD, REF | Nominal schema identity, other keys/scalars, dynamic lists, sets, windows, owned values |

The compiler remains Lexer → Parser → Checker → Emitter. The checker owns
normalization, validity analysis, candidate and target selection. The emitter
receives resolved calls, handler guards, state layouts and concrete bindings;
it must not recreate overload resolution or inspect C++ bodies.

## First port

`hgl-native` provides `bit_and_i64` and four borrowed endpoint queries.
`hgl-stdlib` contains hand-written lowering specimens for `bit_and<i64>`,
`sample<i64>` (an i64 signal input) and `dedup<i64>`. These are executable
fresh-run ports, not compiler-generated implementations or replacement core
registrations. `dedup` recovery remains blocked by NAT-5.

The [cases](stdlib/cases.json) give reasoning before execution. Expected output
cells mean **ticks**, not held values; `null` means no tick. The replay compares
released Python operators, current C++ operators and the HGL C++ projection.
Borrowed-view tests additionally reuse accepted recursive runtime rules;
existing [variations](../runtime_spec/validation/fixed/README.md) still apply.

## Porting order

Upstream's catalogue at `73cc53c97c54079e245e538ae61a3709012ba933` contains 210
core identities: 26 implemented, 44 partial domains, 6 native providers and
134 blocked **in upstream HGL**, not in Rust.

1. Reuse accepted HGL module parts and their tests. Port scalar helpers and
   default/explicit `when`, passive inputs, `inject out`, state and lifecycle.
2. Implement checked Rust target mappings. Exercise scalar values, borrowed
   recursive views and a throwing helper before claiming a stable binding ABI.
3. Port collection nodes against TSD/REF/nested-graph traces, including removed
   members and rebinding. Add typed views without flattening assemblies.
4. Add owned scalar/container types, sets/windows and recovery capabilities;
   then domain providers and higher-order operators. Each catalogue entry names
   its supported domain, evidence and blocker; never mark a family complete
   from one specialization.

Upstream references: `language/stdlib/hgl/hgraph/{operators,stream,native}.hgl`,
`language/stdlib/catalogue/README.md`, `language/docs/design/native-interface.md`
and ADR 0008, “Language contracts and target mappings”. ADR 0014 settles shared native interfaces and source-owned implementations;
collection-view spelling and the full migration remain tracked separately.
