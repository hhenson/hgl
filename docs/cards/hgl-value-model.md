# Card: hgl-value-model

Syntax-neutral ordinary metadata, checked IR definitions and contextual scalar
materialization. Owns diagnostics, retained literal forms, nominal and ordinary
shapes, time context and checked IR. Source spelling and literal parsers stay in
hgl-source; source admission stays in hgl-semantics. Checked type facade paths remain; parsing callers must migrate to the Source
`TypeSpelling` trait, as documented in the hgl-source card.

May use hgl-types and the existing jiff dependency; cannot depend on source AST,
parsing, semantic binding or compiler emission. Budget 1100. Existing moved
module ceilings are split or transferred without increases.

DEC0004 reason 3: source and model each exceed 1000 lines
under xtask's code-line count. Cargo prevents lower layers from reaching source
parsing and name resolution. The relocation changes no language behavior.
