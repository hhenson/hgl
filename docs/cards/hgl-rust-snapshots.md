# Card: hgl-rust-snapshots

Own the emission-only layouts for independently retained current Tuple and concrete Struct values.
The `snapshots` module has an initial 300 source-line budget within the unchanged
hgl-rust crate limit. It uses layouts and checked IR; no source types, runtime
schema lookup, reference semantics, or external dependencies are added.

Public surface: `storage`, `children`, `complete`, `publish`, `bounds`, `ordinary`.
`storage` gives each tuple position an Optional typed slot; list positions and
live map children have independently optional descendants. The NUL-prefixed
synthetic identities cannot be source declaration names. Canonical Tuple, Struct, List,
and Map identity remains in Value.ty; the IR snapshot flag records physical
representation only. Ordinary complete constructors keep their existing native
layout. The existing value emitter evaluates and retains elements once in written order
before assembly; failure prevents later evaluation, preserving earlier effects.
`complete` converts complete native children without supplying absent defaults.
`bounds` derives capacities from existing scalar limits and finite input domains.

`children` returns declared positional child types during cold emission. The private
Struct identity carries its exact canonical nominal type as emission metadata;
field order, generic arguments and optional declarations are preserved. `publish`
delegates complete value output to structural_publication; sparse delta emission
stays separate. That owner reconciles fixed child validity and Map membership
under the pinned bounded complete-publication contract. Existing Tuple growing
List retention remains its previous present-prefix path, with no shrink claim.
No empty event or scheduling rule is introduced.

Acceptance: published ordinary-tuple example and shared retained Tuple scenarios,
including initial unset positions, actual false, nested Tuple/List/Map values,
independent copies, and complete native constructor ordering.

Owning observation adapters require the existing finite prepared execution proof;
standalone/unproved private-slot use rejects before emission. Atomic ordinary
Tuple results retain whole-value publication and cannot accept sparse snapshots.
