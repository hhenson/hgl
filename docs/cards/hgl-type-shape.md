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
