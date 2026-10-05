# Card: hgl-optional

Status: implemented semantic checkpoint; finite evaluation allocation work remains.

Internal ordinary field-presence storage. May use hgl-global-value and hgl-types;
budget 160 source lines. No source nullable type, optional read, clearing or
mutation operation is introduced.

`Optional<T: GlobalValue>` implements GlobalValue with `Value=Option<T::Value>`,
`Slots=usize` and `WIDTH=1`. The slot names a zero-or-one-element descriptor.
`OrdinaryType::OptionalField` records exact field presence in construction-time
schemas. None constructs, retains and reads no payload. Some delegates recursive
owning retention to T. Commit replaces presence and fully reclaims the previous
payload; it never merges old fields into a new snapshot.

Preparation obtains descriptor layouts and all descendant capacity before
commit. The current owning preparation/read path can allocate; the generated
finite evaluation allocation test remains a failing prerequisite gate. Do not
claim zero-allocation complete publication until that gate passes.

Acceptance: the six unchanged optional std cases execute in debug and release,
covering all-unset present values, present zero/empty values, replacement,
independent nested ownership, sparse atomic children and cross-run replay.
