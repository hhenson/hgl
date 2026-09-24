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

C++ supports concrete scalar value interfaces, overloads and `throws`; 31 core
scalar helpers now use the generated adapter. Rust trait emission currently
supports concrete bool/i64/f64 declarations without overloads or `throws` and
rejects unsupported contracts. Temporal native interfaces are recognized but
cannot use this scalar ABI; their provider ABI and the collection-view
migration remain outstanding. Existing inline view helpers retain their legacy
behaviour until that migration is settled.
