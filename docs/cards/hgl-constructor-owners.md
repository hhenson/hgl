# Card: hgl-constructor-owners

The constructor_owners module of hgl-semantics prepares private canonical storage for admitted owning struct, recursive and concrete-family constructor locals. It may use ir and ir_visit; initial budget 100 lines within the unchanged semantics crate cap.

Public surface: prepare, with statically selected ownership and canonical conversion callbacks. Each local and immutable copy receives its own private typed entry and retains the original finite constructor bounds. It adds private typed entries and rewrites lexical reads and checked mutable targets to typed borrowed slots. Mutable constructor assignment replaces the same private owner, while earlier copies and boxes keep their independently retained values. Closed owning literal subtrees become immutable configuration; native calls, value calls and dynamic operands retain their original evaluation phase. Helper formal scopes and captured harness values are not rewritten. Source admission and public IR meaning remain unchanged.

Acceptance: a recursive local can be projected ordinarily and then boxed without native intermediate allocation; provider operands execute once in written order, and selected boxed fields remain independent after another box changes.

The constructor_growth sibling (initial ceiling 40, uses ir and ir_visit) identifies mutable roots whose children are pushed. Those roots retain the existing dynamic storage policy because initial constructor bounds do not prove descendant growth. No HGL admission changes. constructor_owners uses this metadata; its ceiling and the crate ceiling remain unchanged. The existing ordinary nested-list growth controls cover this boundary.
