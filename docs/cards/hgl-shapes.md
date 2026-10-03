# Card: hgl-shapes

Prepared compile-time shape proofs for recursive endpoints. Uses hgl-types and
hgl-bindings; budget 350 source lines. No unsafe or third-party dependencies.

Public contracts: `Shape::shape() -> TsType`; `Field<const N>::Child` and
`Elements::Child` identify statically selected children. `Input<S>` and
`Output<S>` are Copy tokens. `bind(&Bindings,id)` validates the complete shape
only during construction. `id()` and output `generation()` expose identity.
`field<N>`, `index`, and `member` project already checked children by positions
or membership keys without inspecting type metadata. `Fixed<S,N>`, `Map<S>` and
`Set<K>` are shape markers; generated nominal/tuple shapes implement the same
traits. Scalar8 implement Shape directly. Fixed index bounds use static N or
prepared child vector bounds; map absence returns None. Output generations
remain attached to projected handles. No borrowed payload or runtime registry.

Acceptance: wrong root shape rejected before hooks; typed nested projections
preserve identity and lifetime, allocate nothing, and never search field names.
Mutants: accept a mismatched root, project wrong fixed position, drop generation.

Generated or native Shape/Field implementations must describe the same child
layout; typed projection uses that compile-time obligation. Output projection
debug-checks the retained parent generation before deriving a child token,
matching scalar Store handle lifetime checks.
