# Card: hgl-rust-prepared-hooks

The prepared_hooks module owns prepared publication and ordered global append/set emission. It may use layouts, values, finite_domains, any_values, scalars, observed, execution_proof, direct_deltas, source_slots, snapshot_slots and any_prepared, any_expression, constructor_fields; initial budget 180 lines.

Private surface: publication, append, replace and set, consumed by the values emitter. Static source projections are evaluated before mutable storage borrowing. Appends reserve the destination, check/copy each typed field and commit length last. Inline Any construction encodes directly into its reserved child owner; complete aggregate inputs use typed source slots and nested boxes flatten without allocating an intermediate owner.

Acceptance: generated successful-evaluation allocation probes for Any scalar, nested struct-field, flattened box and complete atomic-list appends; existing finite ordinary global hook behavior remains unchanged.

Prepared lexical replacement checks and copies complete typed source slots into existing reserved owners. It preserves capacity through sparse or empty replacements so later larger observations cannot lose provisioned child storage. Growth bounds include one start publication plus the finite evaluation horizon.
