# Native implementation interfaces

Status: agreed model; scalar bindings implemented with upstream ADR 0014.

The HGL declaration owns typing, temporal role, borrowing, effects and errors.
hgraph owns C++ implementation parts and providers; hgl owns Rust parts and
providers. Share HGL contracts across repositories and select one implementation.
Implementations live in ordinary native source. Generate a C++
`bind<Implementation>()` adapter and a Rust implementation trait from that
contract. Native-library builds own dependency configuration; do not author a
second per-function symbol manifest.

- `native fn`: time-series inputs and result; `const` parameters remain fixed
  configuration. All-`const` inputs do not make the function value-level.
- `native const fn`: ordinary value-function typing and no independent ticks.
  A scalar helper cannot implement a temporal declaration merely because its
  payload types match.
- Use the same checked function contract for HGL and native bodies. Binding
  selects an implementation; it does not define another function kind.
- Borrowed endpoint access must be explicit. Do not reinterpret ordinary
  collection values as live input views. Its source spelling remains pending.
- Resolve overloads and normalize substituted REF types before binding checks.
  Preserve the selected contract in IR; select the provider when building
  its library. No per-tick name lookup.
- Reject missing members, convertible-but-wrong signatures, incompatible
  temporal roles and error policies. Native compilation checks ABI shape;
  language checks retain phase, validity and lifetime obligations.

The scalar contract for the initial port is:

```hgl
module hgraph.native
native const fn bit_and(lhs: i64, rhs: i64) -> i64
```

C++ implements the generated interface with a static member and `bind<T>()`;
Rust implements its generated trait. Both implement `lhs & rhs`. The temporal
`hgraph.operators.bit_and` node retains its input handles, activation and output
publication and calls this value helper during evaluation.

Acceptance: preserve the reasoned/Python/C++/Rust bitwise traces in
[stdlib cases](stdlib/cases.json), including missing and duplicate ticks.
Compile-fail cases cover wrong argument/result types, temporal-to-scalar
binding, missing members and borrowed-view escape. Keep generated interface
checks separate from runtime trace evidence.

## Implementation parts and injectables

Keep the callable signature in the shared interface:

```hgl
module std.native
native fn filter(value: i64, const limit: i64) -> i64
```

Select a target part in the library build. Its body describes implementation
requirements. `{}` means a native graph; `{ when; }` means a native node:

```hgl
module std.native part cpp_impl
native fn filter(value: i64, const limit: i64) -> i64 {
    inject out, logger
    start;
    when;
    stop;
}
```

The implementation completes exactly one matching declaration; it is not an
overload. Match parameter names, types, constness, generics, result and exception
policy. Reject duplicate declarations, selected implementations or hooks.
`start;` and `stop;` require `when;`. Part names have no target-selection magic.
An unnamed shared interface may accompany named parts.

`native const fn` remains a value helper. Its selected body may request logger
or clock but cannot declare node lifecycle, output or scheduler ownership.
C++ and Rust parts may request different services. After selection, requirements
silently propagate through value-helper callers, transitively and without
duplicates. Observable behaviour remains the shared function's obligation.
Unavailable services are checking errors.

`out` denotes the declared temporal result, not another output. Constructed
nodes own their output, scheduler and lifecycle; graph-construction callers do
not borrow those capabilities. Nested nodes follow engine activation/teardown.
Native constructors/destructors and Rust `Drop` do not replace start/stop.

C++ value adapters support logger/clock; generated Rust traits support logger.
The compiler preserves temporal graph/node shape and hooks. Their execution ABI
and explicit borrowed-TS helper spelling remain pending; backends reject those
contracts instead of emitting scalar calls.

A node calls a value helper directly during evaluation:

```hgl
fn describe_each(value: i64) -> str {
    when { return describe(value) }
}
```

The helper receives the current scalar value and borrows the enclosing node's
logger. Its string result is published by the enclosing `return`; the helper
creates no node or output. Its requirements contribute to the node's contract.
Calling a temporal `fn`, native or HGL, instead belongs to graph construction
and is rejected inside `when`. A helper mutating its caller's output requires
explicit borrowed access; that spelling remains unsettled.

The upstream [implementation-part rules and acceptance cases](https://github.com/hhenson/hgraph/blob/codex/native-implementation-parts/language/docs/design/native-implementation-parts.md)
cover matching, selection, ownership and lifecycle diagnostics. Existing
[value-helper traces](capabilities/README.md) remain the reasoned/Python/C++
oracle. Rust generated-trait tests prove binding shape and borrowing, not
compiler-generated node-context execution.

## Implemented slice

The upstream compiler's `emit-native-rust` command generates
`crates/hgl-native/src/scalar_interface.rs` from the shared
`crates/hgl-native/interfaces/scalar.hgl` declaration and selected
`scalar-impl.hgl` requirements. Only the shared declaration tracks upstream;
the Rust implementation part is maintained here.
`StandardNative` implements that trait; the existing node calls it through `bit_and_i64`. No symbol manifest or
third-party dependency is introduced.

```sh
python tools/native_bindings.py --compiler <hgl> --interface <upstream-interface> --implementation crates/hgl-native/interfaces/scalar-impl.hgl
python tools/native_bindings.py --compiler <hgl> --check
cargo xtask ci
```

CI builds the upstream compiler at the revision pinned in `ci.yml` and checks
the vendored declaration against upstream and generates the Rust trait using
the local implementation part. Supply interface and implementation paths together;
they need not share a repository. Update the pin and regenerate together when
changing the shared contract.

C++ supports concrete scalar value interfaces, overloads and `throws`; 56 core
scalar helpers now use the generated adapter. Rust trait emission currently
supports concrete bool/i64/f64 declarations without overloads or `throws` and
rejects unsupported contracts. Temporal native interfaces are recognized but
cannot use this scalar ABI; their provider ABI and the collection-view
migration remain outstanding. Existing inline view helpers retain their legacy
behaviour until that migration is settled.
