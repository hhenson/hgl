# Card: hgl-value-check

Ordinary value authority, borrow provenance and lexical entry-effect checking
for checked HGL hook IR. Uses only `hgl-source` and `hgl-rust-ir`; budget 400
source lines. No runtime or third-party dependencies.

Public surface: `ordinary`, `writable`, `field`, `provenance`, `binding`,
`helper_argument`, `validate`, `list_literal`, `indexed`, `list_operation`. The frontend resolves names, types and const
keys first, then uses these checks before emitting a plan.

`binding` preserves aggregate get provenance and enforces alias permissions;
`provenance` applies only to aggregate views, so primitive extraction is owned.
`validate` checks each lifecycle hook independently, retaining declared borrow
lifetimes through the lexical block even when a name is shadowed. Conflicts
use prepared entry indices after equal configured strings have been unified.
Constructors and global set retain their values; ordinary helpers and output
cannot retain live aggregate view authority. No runtime registry is introduced.

Acceptance: aggregate global source checking and executed fixtures in
`hgl-program`, including disjoint scopes, const-key aliases, recursive
readonly authority, exclusive aliases and explicit retention boundaries.

Ordinary list literal checking requires homogeneous constant elements and exact
contextual fixedness; empty literals need concrete context. Indexed aggregate
projections inherit root authority/provenance. Primitive projection and length
results are owning scalar observations. Push checks unboundedness, writable
receiver authority and exact item type, then supplies an explicit retaining
operation to the backend. Indexed replacement is outside this contract.

Uncontextualized nonempty ordinary literals are explicitly unsupported: the
pinned specification describes constant homogeneous list literals but does not
uniquely define their inferred fixedness. Contextual nonempty literals retain
their expected exact list type. Harness sequence typing is unchanged.
