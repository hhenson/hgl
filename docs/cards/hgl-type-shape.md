# Card: hgl-type-shape

Canonical checked source types and finite nominal application identities. No
runtime or third-party dependency; budget 300 source lines.

Public surface: `Ty`, `Ty::{name,parse,list_parts,source_name}`;
`Nominal { origin, arguments }`, `Nominal::source_name`, conversions from String
and str for nongeneric origins; `application` splits a complete type application
without losing nested arguments. Ty retains ordinary list fixedness and fully
substituted struct fields. Nominal retains the declaring module-qualified origin
and complete invariant checked source argument shapes, including phantom args.
Canonical source names identify specializations without importer aliases.

Acceptance: compiler generic constructor/import/inference tests and distinct
prepared global schemas for equal-layout specializations.

Structural publication shapes add `Ty::{Map,Tuple}` and ordinary
`Ty::Delta(origin)`. `Ty::publication` admits exactly the finite recursive
scalar8/bool-i64-set/fixed-list/tuple/nominal/i64-key-map profile.
`Ty::delta` validates that profile and reduces scalar origins to themselves;
structural origins retain their complete exact shape. Delta is never itself a
temporal publication shape. `delta_argument` recognizes only the contextual
outer `delta<type>` spelling. These rules derive from spec321ba4b
ordinary-delta-types and contextual-collection-deltas.

`Ty::Atomic(Box<Ty>)` preserves a composite whole-value temporal boundary.
`Ty::atomic(self) -> Self` normalizes every admitted scalar leaf to itself;
`Ty::atomic_payload(&self) -> bool` admits finite ordinary scalar8, lists,
tuples and concrete required/defaulted structs recursively. Publication
admission includes such atomic shapes, and `delta` reduces their payload to V.
Canonical generic arguments normalize before occurrence checks and matching;
an ordinary composite V alone never determines an atomic origin for `delta<T>`.
These rules follow the atomic-scalar-equivalence and atomic-delta-publications
contracts. Unsupported source scalar families remain explicit diagnostics.

## Temporal scalar preparation

Add CivilDateTime, TimeZone, ZonedDateTime and ZonedTime scalar variants and source
spellings; scalar atomic normalization and delta reduction apply identically.
The same twelve leaves are admitted recursively under the pinned publication
profile. ZonedTime uses source spelling zoned_time; its exact identity is wall-clock
time plus exact zone name, with no date or offset. Existing non-key shape restrictions remain.

May use hgl-type-syntax for pure spelling decomposition, preserving existing
application/delta_argument exports and Ty::list_parts. See its card.

EnumType { origin: String, members: Vec<(String, i64)> } owns a validated
nominal enum declaration. Ty::Enum(EnumType) is a scalar publication leaf; its
source name is the canonical origin. Atomic normalization and delta reduction
apply identically.

Scalar-collection-keys (spec06e576a) admits every built-in scalar and declared enum
as the exact K of set<K> and map<K,S>. Composite/reference/native keys remain
outside the profile. f64 value restrictions belong to key checking, not type
formation. No key normalization changes the declared source type.

apply identically; enum keys and set members remain excluded.

Optional atomic structs retain `Ty::Struct(Nominal, Vec<(String,Ty)>, Vec<usize>)`;
the final vector contains optional declaration positions. It is schema metadata,
not a nullable source type. Ordinary projection preserves it. Complete atomic
payload admission permits optional fields recursively; structural publication
roots with optional fields remain outside this admission. Delta of an atomic
struct is its complete ordinary value, including field presence.

RecursiveType aliases immutable Batch<Nominal,Ty>; NominalDefinition aliases its
ordered definition record. Ty::Recursive carries a complete finite batch at every
standalone root and nominal references on recursive edges. Identity/order compare
exact Nominal only; field metadata never changes identity. Such roots are ordinary
atomic payloads, never structural publication roots. StructFields and
Ty::structure()->Result<StructFields,String> resolve complete root fields and
presence positions; a bare internal edge is an explicit unresolved-schema error.
May use hgl-nominal-batch. Source formation follows spec6baa056/ADR0012.

FamilyType aliases immutable Family<Nominal,Ty>. Ty::Family is an ordinary atomic
payload retaining declaration-fixed concrete members and explicit ancestor
specializations. Its source identity is the exact applied abstract declaration;
structural family publication remains unsupported. Family fields are not exposed
through Ty::structure. This is the nonrecursive family slice of spec97c791f.

atomic_payload also admits ordinary Set and Map whose K satisfies collection_key, with recursively admitted ordinary V. This does not change their unwrapped temporal publication shapes.
collection_key() admits scalar leaves, finite positional tuples and concrete
structs recursively, including optional fields. Recursive/family/collection/ref
components remain excluded. Temporal Set/Map publication uses this same complete
ordinary K classification; atomic payload classification remains independent.
