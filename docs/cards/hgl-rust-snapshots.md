# Card: hgl-rust-snapshots

Own the emission-only layouts for independently retained current Tuple values.
The `snapshots` module has an initial 300 source-line budget within the unchanged
hgl-rust crate limit. It uses layouts and checked IR; no source types, runtime
schema lookup, reference semantics, or external dependencies are added.

Public surface: `storage`, `complete`, `publish`, `bounds`, `ordinary`.
`storage` gives each tuple position an Optional typed slot; list positions and
live map children have independently optional descendants. The NUL-prefixed
synthetic identities cannot be source declaration names. Canonical Tuple, List,
and Map identity remains in Value.ty; the IR snapshot flag records physical
representation only. Ordinary complete constructors keep their existing native
layout. The existing value emitter evaluates and retains elements once in written order
before assembly; failure prevents later evaluation, preserving earlier effects.
`complete` converts complete native children without supplying absent defaults.
`bounds` derives capacities from existing scalar limits and finite input domains.

Output publication follows existing present-entry application. An unset owned
child is omitted; it never becomes zero, false, or implicit invalidation. A
structural Map observation publishes its live children without deriving removals
from missing keys. Ordinary held-map replacement and later nil-copy invalidation
are outside the present structural publication contract
(ordinary-delta-types.md, excluded transitions; ordinary-tuple-values.md, output
rules unchanged). No empty event or scheduling rule is introduced.

Acceptance: published ordinary-tuple example and shared retained Tuple scenarios,
including initial unset positions, actual false, nested Tuple/List/Map values,
independent copies, and complete native constructor ordering.

Owning observation adapters require the existing finite prepared execution proof;
standalone/unproved private-slot use rejects before emission. Atomic ordinary
Tuple results retain whole-value publication and cannot accept sparse snapshots.
