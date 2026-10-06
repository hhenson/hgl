# Card: hgl-prepared-value

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `prepared_value` module of `hgl-store` (`crates/hgl-store/src/prepared_value.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Finite ordinary value preparation, using hgl-global-value, hgl-columns and
hgl-types; budget 160 source lines. No unsafe code or third-party dependencies.

`PreparedValue: GlobalValue + Sized` adds an opt-in finite-capacity protocol;
existing GlobalValue implementations remain valid. `Bounds: Default` describes
cold capacity. `include(&mut Bounds,&Value)` merges native maxima, and
`allocate(&mut ValueColumns,&Bounds)->NodeResult<ValueSlot<Self>>` creates an
independently owned destination. It does not invent root presence/publication.

`check_native(&ValueColumns,ValueSlot<Self>,&Value)->NodeResult` and
`check_slots(&ValueColumns,ValueSlot<Self>,&ValueColumns,ValueSlot<Self>)->NodeResult`
preflight every descendant before any live field changes. After preflight,
`copy_native(&mut ValueColumns,ValueSlot<Self>,&Value)`,
`copy_between(&ValueColumns,ValueSlot<Self>,&mut ValueColumns,ValueSlot<Self>)`,
and `copy_within(&mut ValueColumns,ValueSlot<Self>,ValueSlot<Self>)` copy into
independent retained capacity without allocation. Copy methods require a matching
successful check with unchanged source/destination. Identical source/destination
slots are valid no-ops. Arbitrarily overlapping descendant roots are not admitted.

Scalars implement the trait via their statically selected column and ScalarCopy;
Bounds is usize (maximum owning byte count, zero for fixed-width scalars).
Composite marker implementations are generated, preserving exact types and field
order. They perform complete preflight before invoking any field copy. Ordinary
owning extraction remains explicit and may allocate outside evaluation.

Acceptance: first publication, empty/short/large repeated replacement, nested
lists, exact provider names, independent source/record captures, capacity rejection
before mutation and zero allocation through native finite publication/capture.
The generated graph.evaluate gate separately verifies the complete compiler path.
