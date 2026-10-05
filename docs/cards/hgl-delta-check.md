# Card: hgl-delta-check

Validate bounded structural delta constructor syntax against an already checked
origin shape. Dependencies: hgl-source, hgl-rust-ir and hgl-scalar-keys; budget 350 source lines. Semantic
authority: spec06e576a scalar-collection-keys, ordinary-delta-types and contextual-collection-deltas.

Public surface: `Part::{Added,Removed,Keyed,Child}` and `constructor(origin,args,fixed)`.
Added/Removed hold ParsedLiteral scalar members/keys; Keyed holds an exact
ParsedLiteral map key, child delta type and borrowed payload. Child retains only
fixed i64 field/position indices, child delta type and borrowed payload AST.
The returned parts preserve supplied argument and sparse-entry order. The
callback returns ParsedLiteral for each constant expression without admitting runtime positions.
Before returning, validate the whole constructor: exact argument names,
duplicates, constant types, disjoint memberships/keys, fixed bounds and shape.
Empty ordinary data is valid; application profile checks belong to the runtime.
Scalar constructors, null and dynamic lists/positions are rejected. This layer
does not execute payloads, publish updates, infer shape from contents or admit
general sparse literals. The compiler checks all returned payloads against
their exact types before any phase evaluates the resulting constructor.

Acceptance: shape-family tests, malformed duplicate/overlap/bounds tests and
source-order payload tests, independent of runtime publication.

values(parts, check_payload) checks each child in written order and constructs
DeltaEntry IR, retaining contextual key recipes without provider lookup.
materialized(&[DeltaEntry]) checks resolved scalar identities for duplicate and
overlap failure during cold evaluation, before publication. Known values use
hgl-scalar-keys equality during checking; recipe text never establishes equality.
Every collection member/key requires exact K with no numeric widening. NaN is
explicitly unsupported. Fixed list/tuple indices remain bounded constant i64.
