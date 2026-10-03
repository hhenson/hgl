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
