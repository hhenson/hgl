# Card: hgl-rust-prepared-hooks

The prepared_hooks module owns prepared publication and ordered global append/set emission. It may use structural_publication, layouts, values, finite_domains, any_values, scalars, observed, execution_proof, direct_deltas, source_slots, snapshot_slots and any_prepared, any_expression and constructor_fields; initial budget 180 lines.

Private surface: publication, append, replace, set, write and storage, consumed by the values emitter. Static source projections are evaluated before mutable storage borrowing. Appends reserve the destination, check/copy each typed field and commit length last. Inline Any construction encodes directly into its reserved child owner; complete aggregate inputs use typed source slots and nested boxes flatten without allocating an intermediate owner.

Acceptance: generated successful-evaluation allocation probes for Any scalar, nested struct-field, flattened box and complete atomic-list appends; existing finite ordinary global hook behavior remains unchanged.

Prepared lexical replacement checks and copies complete typed source slots into existing reserved owners. It preserves capacity through sparse or empty replacements so later larger observations cannot lose provisioned child storage. Growth bounds include one start publication plus the finite evaluation horizon.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.

Global set/presence emission lives in prepared_global_writes (initial ceiling 60, uses layouts, prepared_hooks, values, any_prepared, finite_domains and snapshot_slots). The existing prepared_hooks::set facade is preserved. Complete constructor/global replacements share ordered field writes; input scalar leaves borrow readonly observations. All existing ceilings and the aggregate ceiling remain unchanged.

Constructor field publication delegates optional and recursive slot selection to constructor_fields. Ordinary owning scalar sources copy directly from retained typed slots.

The constructor owner uses the private write and storage helpers for ordered
field retention into staging. Replacement then preflights the complete source
before copying to the existing destination. The lower-level regression shrinks
private reservations deliberately: earlier retention failure stops later native
effects, and a later field failure leaves an existing destination unchanged.
This instrumentation does not claim those sources naturally exceed finite bounds.

Prepared global writes recognize private staged constructor sources and commit
through checked slot copies, retaining Native payloads without intermediate
owners. May use constructor_owners for the checked origin predicate.

Complete staged Tuple results publish directly from their canonical global slots
through structural_publication::ordinary. This preserves independent operand
owners and the existing whole-source evaluation order while avoiding an owning
read of variable-width scalar children before publication.
