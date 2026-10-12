# Card: hgl-constructor-owners

The constructor_owners module of hgl-semantics prepares private canonical storage for admitted owning struct, recursive and concrete-family constructor locals. It may use ir and ir_visit; initial budget 100 lines within the unchanged semantics crate cap.

Public surface: prepare, with statically selected ownership, canonical conversion and Tuple staging callbacks. Each local and immutable copy receives its own private typed entry and retains the original finite constructor bounds. It adds private typed entries and rewrites lexical reads and checked mutable targets to typed borrowed slots. Mutable constructor assignment replaces the same private owner, while earlier copies and boxes keep their independently retained values. Closed owning literal subtrees become immutable configuration; native calls, value calls and dynamic operands retain their original evaluation phase. Helper formal scopes and captured harness values are not rewritten. Source admission and public IR meaning remain unchanged.

Acceptance: a recursive local can be projected ordinarily and then boxed without native intermediate allocation; provider operands execute once in written order, and selected boxed fields remain independent after another box changes.

The constructor_growth sibling (initial ceiling 40, uses ir and ir_visit) identifies mutable roots whose children are pushed. Those roots retain the existing dynamic storage policy because initial constructor bounds do not prove descendant growth. No HGL admission changes. constructor_owners uses this metadata; its ceiling and the crate ceiling remain unchanged. The existing ordinary nested-list growth controls cover this boundary.


Owning constructor expressions in evaluation handlers also receive independent
private entries, so assignment and append consume a complete staged value.
Each source operand is evaluated and retained before the next operand begins.
The normalizer does not move expressions out of control flow or helper scopes;
existing helper and start/stop boundaries remain unchanged.
Closed collection recipes retain their literal boundary; staging their nested
constructors must not invalidate the collection's finite capacity proof.

Only the converted outer family owner bypasses whole-value cold hoisting.
Nested literal family coercions retain their exact tag and owning children in
configuration slots; dynamic parent fields still execute once at their original
position. Ordinary and recursive parent controls cover text and list descendants.

Closed subtree selection and retention delegate to value_constant; this module
retains lexical owner identity, expression position and finite constructor origins.

Local owner discovery includes generator bodies and their nested control flow.
Generator locals use the same independent destinations and ordered initialization
as hook locals; preparation never executes their operands or advances resumption.

The target-provided Tuple predicate stages atomic results and complete layouts
with a proved prepared ordinary publisher. Other Tuple constructors retain their
existing snapshot transport, rather than hiding it behind an owning canonical
read. The predicate changes only private preparation, never source admission.
