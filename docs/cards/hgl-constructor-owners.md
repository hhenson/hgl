# Card: hgl-constructor-owners

The constructor_owners module of hgl-semantics prepares private canonical storage for admitted owning struct and recursive constructor locals. It may use ir and ir_visit; optional reporting target 100 lines.

Public surface: prepare. It adds private typed entries and rewrites lexical reads to readonly borrowed slots. Closed owning literal subtrees become immutable configuration; native calls, value calls and dynamic operands retain their original evaluation phase. Helper formal scopes and captured harness values are not rewritten. Source admission and public IR meaning remain unchanged.

Acceptance: a recursive local can be projected ordinarily and then boxed without native intermediate allocation; provider operands execute once in written order, and selected boxed fields remain independent after another box changes.

Owning constructor expressions in evaluation handlers also receive independent
private entries, so assignment and append consume a complete staged value.
Each source operand is evaluated and retained before the next operand begins.
The normalizer does not move expressions out of control flow or helper scopes;
existing helper and start/stop boundaries remain unchanged.
Closed collection recipes retain their literal boundary; staging their nested
constructors must not invalidate the collection's finite capacity proof.
