# Card: hgl-delta-check

Validate bounded structural delta constructor syntax against an already checked
origin shape. Dependencies: hgl-source only; budget 350 source lines. Semantic
authority: spec60a2d7e ordinary-delta-types/contextual-collection-deltas.

Public surface: `Part::{Added,Removed,Child}` and `constructor(origin,args,fixed)`.
Added/Removed hold constant scalar members or integer map keys. Child holds a
checked field/position/key, exact child delta type and borrowed payload AST.
The returned parts preserve supplied argument and sparse-entry order. The
callback resolves each constant expression without admitting runtime positions.
Before returning, validate the whole constructor: exact argument names,
duplicates, constant types, disjoint memberships/keys, fixed bounds and shape.
Empty ordinary data is valid; application profile checks belong to the runtime.
Scalar constructors, null and dynamic lists/positions are rejected. This layer
does not execute payloads, publish updates, infer shape from contents or admit
general sparse literals. The compiler checks all returned payloads against
their exact types before any phase evaluates the resulting constructor.

Acceptance: shape-family tests, malformed duplicate/overlap/bounds tests and
source-order payload tests, independent of runtime publication.
