# Card: hgl-scalar-copy

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `scalar_copy` module of `hgl-store` (`crates/hgl-store/src/scalar_copy.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Capacity-preserving scalar ownership, depending only on hgl-types; budget 180.
No unsafe code, type erasure, reference counting or third-party dependencies.

`ScalarCopy: Clone + Default` provides `size()->usize`, `capacity()->usize`,
`reserve(usize)->NodeResult`, and prevalidated `copy_from(&mut self,&Self)`.
Fixed scalar implementations have zero variable bytes and copy directly.
String, ZoneId, ZonedTime and ZonedDateTime reserve and preserve their independent
owned text buffers; their copies preserve exact name spelling and all value fields.
Reserve is cold and fallible; successful hot copying requires sufficient capacity,
as checked by PreparedValue or the prepared scalar publication facade.

The sealed Scalar storage trait inherits this contract. There is no per-value
type test and no replacement of the destination with a source-owned backing token.
Acceptance includes empty names/text, longer replacements within bounds, exact
alias distinction, retained capture independence and zero measured hot allocations.
