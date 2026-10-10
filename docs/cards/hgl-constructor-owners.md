# Card: hgl-constructor-owners

The constructor_owners module of hgl-semantics prepares private canonical storage for admitted owning struct, recursive and concrete-family constructor locals. It may use ir and ir_visit; initial budget 100 lines within the unchanged semantics crate cap.

Public surface: prepare, with statically selected ownership and canonical conversion callbacks. Each immutable copy receives its own private typed entry and retains the original finite constructor bounds. It adds private typed entries and rewrites lexical reads to readonly borrowed slots. Closed owning literal subtrees become immutable configuration; native calls, value calls and dynamic operands retain their original evaluation phase. Helper formal scopes and captured harness values are not rewritten. Source admission and public IR meaning remain unchanged.

Acceptance: a recursive local can be projected ordinarily and then boxed without native intermediate allocation; provider operands execute once in written order, and selected boxed fields remain independent after another box changes.
