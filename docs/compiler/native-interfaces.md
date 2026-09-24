# Native implementation interfaces

Status: agreed model; scalar bindings implemented with upstream ADR 0014.

The HGL declaration owns typing, temporal role, borrowing, effects and errors.
C++ and Rust implementations live in ordinary native source. Generate a C++
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

## Outputs and injectables

Value-helper injection and inference are implemented in the upstream compiler.
C++ supports `logger` and `clock`; generated Rust traits support `logger`.
Temporal native bindings below remain an agreed extension:

```hgl
native fn accumulate(value: i64) -> i64 {
    inject out, logger
}

native const fn describe(value: i64) -> str {
    inject logger
}
```

`out` is `Output<T>` derived from the temporal result `-> T`; it is not an
additional output. `logger`, `clock` and `scheduler` have capability types
`Logger`, `EvaluationClock` and `Scheduler`. These are call-scoped access to
services or endpoints, not payloads to temporalize. Public type spelling and
target wrappers remain implementation work.

`const fn` is non-temporal, not pure. A logger may be supplied by its call
context without creating a node. Missing context is a diagnostic. It cannot
inject its own temporal output, scheduler or node state. Clock access requires
a runtime context and an admitted phase. Forwarding a caller's node capabilities
needs an explicit ownership contract and remains unsettled.

The portable contract records requirements, including through helper calls and
imports. Calls silently upgrade the caller's injectable list, transitively and
without duplicate requests. The caller need not repeat `inject logger`. Each target binding records the subset its implementation uses, in
native source. C++ may request `out, logger` while Rust requests only `out`;
the adapters may therefore have different parameter lists. Provision only the
used facilities, but check calls against the portable contract on every target.
Observable effects promised by that contract remain obligations on all targets.
Provider-private allocators and scratch storage need no HGL declaration. Extra
semantic capabilities must be declared; unavailable capabilities are errors.

Illustrative signatures when both capabilities are used, not current APIs:

```cpp
static void accumulate(const Input<Int>& value, Output<Int>& out, Logger& logger);
static String describe(Int value, Logger& logger);
```

```rust
fn accumulate(value: Input<'_, i64>, out: Output<'_, i64>, logger: Logger<'_>);
fn describe(value: i64, logger: Logger<'_>) -> String;
```

Binding checks the selected target signature against the shared contract. A
temporal implementation's native void/unit return does not erase its HGL
output: this form publishes through `out`. Capability access cannot escape the
call. Injection alone must not classify an HGL function as a runtime node.

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

The upstream [capability contract and acceptance cases](https://github.com/hhenson/hgraph/blob/codex/native-interface-bindings/language/docs/design/decisions/0014-native-implementation-interfaces.md#outputs-and-capabilities)
cover function parity, output shape, missing context, target subsets, imports,
phase and lifetime errors, logging effects and nested output deltas. The
[value-helper reference traces](capabilities/README.md) agree with reasoning in
Python and C++. Compiler tests cover silent transitive inference, deduplication,
imports, lifting and missing runtime context. Temporal provider output and
target-specific subsets remain pending; current adapters pass all declared
capabilities. Rust node-context lowering is not implemented by the trait test.

## Implemented slice

The upstream compiler's `emit-native-rust` command generates
`crates/hgl-native/src/scalar_interface.rs` from
`crates/hgl-native/interfaces/scalar.hgl`, a vendored copy of the upstream
`native/scalar_values_i64.hgl` module part. `StandardNative` implements that
trait; the existing node calls it through `bit_and_i64`. No symbol manifest or
third-party dependency is introduced.

```sh
python tools/native_bindings.py --compiler <hgl> --interface <upstream-interface>
python tools/native_bindings.py --compiler <hgl> --check
cargo xtask ci
```

CI builds the upstream compiler at the revision pinned in `ci.yml` and checks
both the vendored HGL part and generated Rust trait. Update the pin and
regenerate together when changing the contract.

C++ supports concrete scalar value interfaces, overloads and `throws`; 56 core
scalar helpers now use the generated adapter. Rust trait emission currently
supports concrete bool/i64/f64 declarations without overloads or `throws` and
rejects unsupported contracts. Temporal native interfaces are recognized but
cannot use this scalar ABI; their provider ABI and the collection-view
migration remain outstanding. Existing inline view helpers retain their legacy
behaviour until that migration is settled.
