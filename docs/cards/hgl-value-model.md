# Card: hgl-value-model

Canonical ordinary type/value metadata and contextual scalar materialization form
an enforced lower layer. Public modules and their existing contracts are
`diagnostics`, `literals`, `nominal_batch`, `type_shape`, `scalar_shape`,
`ordinary_shape`, `type_syntax`, `time_context` and `execution_phase`. Root exports
retain `Issue`, scalar literal forms, `Ty`, nominal metadata and window metadata.
The source crate reexports these paths for existing clients.

May use hgl-types and the existing jiff dependency. Cannot reach source tokens,
Cursor, Expr, declaration discovery, semantic Library or executable native images.
The aggregate budget is 1200; the moved module ceilings remain unchanged.
Existing source/model tests continue exercising the same public facade.

This boundary implements DEC0004 reason 3: the model and parser sides each exceed
1000 lines, and Cargo prevents ordinary type/value operations from reentering
source parsing or name resolution. Contextual scalar construction uses only its
explicit execution context; native activation remains outside this layer.
