# Card: hgl-native

Status: bounded Rust binding probe; [contract](../compiler/node-authoring.md).

Thin native helpers. May use `hgl-store`, `hgl-types`. Budget: 130 lines.
Tests may use `hgl-alloc-count`. No registry, node lifecycle, mutation, scheduler
or third-party dependencies.

Public surface:

```rust
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
NAT-1/2 catalogue validation is future compiler work, not provided by this crate.

Done: bitwise edge cases; scalar/recursive/unbound/assembled view observations;
a compile-fail test for retaining the borrow; allocation checks on queries.
Mutants, on a copy: replace `&` with `|`; use valid for all_valid; report valid
as modified. Each must fail a named test before completion.
