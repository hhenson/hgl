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

`Atomic<T: hgl_global_value::GlobalValue>` is a whole-value endpoint marker;
its Shape is `TsType::Atomic(T::schema())`. This card also permits the
hgl-global-value dependency. Atomic inputs/outputs use the same prepared
Input/Output tokens and generation checks, without structural field projection.

CivilDateTime, ZoneId, ZonedDateTime and ZonedTime are concrete typed scalar implementations.
Zone-bearing values retain exact owned names fallibly at retention boundaries;
prepared borrowed projections neither allocate nor consult a provider.

Map<S,K=i64> and Set<K> require the statically selected hgl-keys Key contract.
Map keeps the i64 compatibility schema; other keys use KeyedDictionary with
exact ordinary identity. Set uses scalar or nominal KeyedSet metadata. Typed
projections continue to use prepared internal membership IDs.

The exhaustive ordinary-schema carrier includes Set/Map variants; source key admission remains the frontend contract.
Growing<S> is the statically typed dense growing-list marker. Its member projections reuse construction-validated typed child tokens; Shape metadata remains distinct from Dictionary and Fixed.
