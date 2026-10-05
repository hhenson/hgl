# Card: hgl-keys

Typed, cold-prepared scalar collection identity. Depends on hgl-types and
hgl-global-value; budget 500 source lines. No provider lookup or erased scalar
values occur here.

`Key: GlobalValue` selects exact `prepare(&mut Keys, &Value) -> NodeResult`,
`id(&Keys, &Value) -> NodeResult<i64>`, `value(&Keys, i64) -> NodeResult<Value>`
and `ids(&Keys) -> &[i64]` operations statically. `Keys` is defaultable run-owned
storage. Preparation retains each distinct owning key once and may allocate.
ID lookup after preparation never allocates and fails explicitly for an unknown
key. Capturing an owning value may allocate under GlobalValue retention rules.

Fixed-width keys use reversible representations with distinct typed domains.
f64 signed zero has one ID; infinities remain distinct; NaN is rejected. Enum
markers delegate physical operations to i64 while endpoint schemas retain the
exact nominal declaration. String and zone-bearing domains retain exact values
and resolve hash collisions by equality. IDs are internal membership tokens,
never a replacement for the ordinary key's typed identity.

Tests cover cold ownership, aliases, zero/infinities, hash collisions, unknown
keys and allocation-free first/repeated lookups after preparation.

`Key::with_value` visits a retained typed key by reference; fixed inline values may use a stack temporary. Owning String and provider identities borrow their cold table entry. It performs no owning capture allocation.

Boolean and i64 keys use their exact inline identity directly for `id`, `value`
and capture; runtime values need no cold registration. `ids` lists only explicitly
registered cold recipes. Other owning key representations keep their existing
prepared retention tables.
