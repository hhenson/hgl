# Card: hgl-delta-check

Validate bounded structural delta constructor syntax against an already checked
origin shape. Dependencies: hgl-source, hgl-rust-ir and hgl-scalar-keys; budget 350 source lines. Semantic
authority: spec443b92c scalar-collection-keys, ordinary-delta-types and contextual-collection-deltas.

Public surface: `Part::{Added,Removed,Keyed,Child}` and `constructor(origin,args,fixed)`.
Added/Removed hold retained checked complete Values; Keyed holds an exact
checked map key, child delta type and borrowed payload. Child retains only
fixed i64 field/position indices, child delta type and borrowed payload AST.
The returned parts preserve supplied argument and sparse-entry order. The
callback returns (Value, Option<Value>) for each constant expression: retained
IR plus any known identity, without admitting runtime positions. Immutable cold
aliases retain their Local IR; metadata must not replay an initializer.
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
materialized(&[DeltaEntry]) checks resolved complete identities for duplicate and
overlap failure during cold evaluation, before publication. Known values use
hgl-composite-keys equality during checking; recipe text never establishes equality.
Every collection member/key requires exact K with no numeric widening. NaN is
explicitly unsupported. Fixed list/tuple indices remain bounded constant i64.

The constructor callback receives (&Expr, &Ty) so ordinary tuple/struct key
construction has exact contextual K. Known complete keys include nominal identity,
field positions and optional presence; partial provider recipes defer identity.
Only structural child positions require a known Literal::Int. Compound key fields
are never treated as sparse delta children.
Growing-list items require constant nonnegative positions; remove accepts constant nonnegative i64 tail positions. Formation rejects duplicates and overlap independently of trace length; tail/gap validation belongs to pre-start publication state.

delta_value checks the exact input endpoint, publication shape and valid/modified facts after the frontend verifies evaluation phase.
