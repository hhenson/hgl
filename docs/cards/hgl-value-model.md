# Card: hgl-value-model

Syntax-neutral ordinary metadata, checked IR definitions and contextual scalar
materialization. Owns diagnostics, retained literal forms, nominal and ordinary
shapes, time context, checked IR and syntax-neutral IR traversal. Source spelling and literal parsers stay in
hgl-source; source admission stays in hgl-semantics. Checked type facade paths remain; parsing callers must migrate to the Source
`TypeSpelling` trait, as documented in the hgl-source card.

May use hgl-types and the existing jiff dependency; cannot depend on source AST,
parsing, semantic binding or compiler emission. Budget 1270 within the unchanged combined 11680-line phase allowance.
The `ir_visit` module owns source-ordered read/mutable expression, statement,
hook and lexical-node walkers with subtree pruning. It never executes values;
helper scope policies stay with callers. `Value::closed` uses this traversal
with its constructor/literal whitelist.

DEC0004 reason 3: source, model and checked execution each exceed 1000 lines
under xtask's code-line count. Cargo prevents lower layers from reaching source
parsing and name resolution. The relocation changes no language behavior.
