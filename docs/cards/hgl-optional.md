# Card: hgl-optional

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `optional` module of `hgl-store` (`crates/hgl-store/src/optional.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Internal ordinary field-presence storage. May use hgl-global-value, hgl-prepared-value, hgl-list-storage and hgl-types;
budget 160 source lines. No source nullable type, optional read, clearing or
mutation operation is introduced.

`Optional<T: GlobalValue>` implements GlobalValue with `Value=Option<T::Value>`,
`Slots=usize` and `WIDTH=1`. The slot names a zero-or-one-element descriptor.
`OrdinaryType::OptionalField` records exact field presence in construction-time
schemas. None constructs, retains and reads no payload. Some delegates recursive
owning retention to T. Commit replaces presence and fully reclaims the previous
payload; it never merges old fields into a new snapshot.

`PreparedValue` uses `Bounds=Option<T::Bounds>`. Cold preparation merges present
payload bounds and retains a vacant child independently of logical presence.
Whole-value checks reject an unprepared present child before copying. Native and
slot copies reuse descendants; clearing internal presence never discards capacity.
The zero-or-one descriptor shares prepared slot copying with list storage while
exposing no list operation. Generated field bounds reserve the finite descendant
capacity needed by all replayed values, including initially absent fields.

The six-case generated test counts every actual graph.evaluate call, including
first publication, pass-through and recording, and passes with zero allocations
in debug and release. Owning GlobalValue extraction remains an explicit cold
retention boundary. Native tests exercise None, Some(empty), repeated replacement,
independent capture and capacity rejection without mutation.

Acceptance: the six unchanged optional std cases execute in debug and release,
covering all-unset present values, present zero/empty values, replacement,
independent nested ownership, sparse atomic children and cross-run replay.
