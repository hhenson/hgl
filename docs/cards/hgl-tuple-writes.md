# Card: hgl-tuple-writes

Own source assignment and control dependence for ordinary Tuple constant-context
checks. The internal tuple_writes module has an initial 100 source-line budget
within the unchanged hgl-semantics limit. It uses source Cursor/Expr, name_check
Scope, tuple_phase dependence and tuple_flow lexical facts.

Public surface: defaultable/clonable/debug `Writes`, `select(&Expr,&BTreeSet<String>)`,
`preserve(&Expr,&BTreeSet<String>,bool)`, `root(&Expr)`, and the Cursor callbacks
`conditional`, `handler`, `assignment`, plus `branch(&mut Scope,&Expr,closure)`.
The callbacks retain existing source expression/body checking. Both if alternatives
inherit the original selector; nested else-if retains enclosing selection.
While and temporal iteration selection affect writes to existing bindings.
Fresh declarations use initializer dependence alone. Handlers retain their existing
execution phase and do not prevent an unconditional fixed replacement within them.

A bare replacement can clear an ordinary variable's runtime provenance.
Compound updates read the previous target. Member/index replacements preserve
root dependence and include runtime index selection. Runtime control conservatively
marks dependent writes. Binding identities and fact merging remain owned by
single tuple_flow Facts; same-spelling inner declarations cannot taint outer values.
This module adds no language forms, type categories, runtime storage or diagnostics.

Acceptance: compound, member/index, if/else/else-if, while and temporal-for updates
cannot erase runtime dependence before an explicit constant Tuple boundary.
Focused negatives require the ordinary uncatalogued Tuple phase message; fixed
updates, unconditional whole resets, fresh guarded constants and lexical shadows
remain accepted by source checking.
