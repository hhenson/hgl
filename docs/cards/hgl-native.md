# Card: hgl-native

Status: scalar provider migration; [contract](https://github.com/hhenson/hgraph_spec/blob/main/language/docs/design/node-authoring.md).

Thin native helpers. May use `hgl-store`, `hgl-types`. Budget: 130 lines.
Tests may use `hgl-alloc-count`. No registry, node lifecycle, mutation, scheduler
or third-party dependencies.

Public surface:

```rust
pub trait Native { fn bit_and(lhs: i64, rhs: i64) -> i64; }
pub struct StandardNative;
impl Native for StandardNative { /* value helper implementation */ }
pub fn bit_and_i64(lhs: i64, rhs: i64) -> i64;
pub struct InputView<'a> { /* immutable call-confined borrow */ }
impl<'a> InputView<'a> {
    pub fn new(store: &'a Store, input: InputId, now: EngineTime) -> Self;
    pub fn valid(&self) -> bool;
    pub fn all_valid(&self) -> bool;
    pub fn modified(&self) -> bool;
    pub fn last_modified(&self) -> EngineTime;
}
```

`new` receives a live input handle from the same store, already checked during
node construction. It neither binds nor reads a payload. `last_modified` is
an engine timestamp; HGL datetime payload storage is a separate missing mapping.

NAT-3/6: queries preserve local assembled-input observations, recursive validity
and accepted REF/removal lifetime. No per-tick allocation or native dispatch.
The upstream HGL compiler generates `Native` from `interfaces/scalar.hgl`.
`StandardNative` implements it in Rust; `bit_and_i64` delegates through the
trait. Regenerate with `tools/native_bindings.py --compiler <hgl>`; `--interface`
and `--implementation` must be supplied together to refresh the vendored upstream
contract. The same pairing applies to `--capability-interface` and
`--capability-implementation`.
The interface is a concrete i64 subset of `hgraph.native`; overload-family,
collection-borrow and fallible Rust bindings remain separate migration work.
NAT-1/2 general catalogue selection remains compiler work.

Done: bitwise edge cases; scalar/recursive/unbound/assembled view observations;
a compile-fail test for retaining the borrow; allocation checks on queries.
Mutants, on a copy: replace `&` with `|`; use valid for all_valid; report valid
as modified. Each must fail a named test before completion.
