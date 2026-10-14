# Card: hgl-value-model

Syntax-neutral ordinary metadata, checked IR definitions and contextual scalar
materialization. Owns diagnostics, retained literal forms, nominal and ordinary
shapes, time context, checked IR and syntax-neutral IR traversal. Source spelling and literal parsers stay in
hgl-source; source admission stays in hgl-semantics. Checked type facade paths remain; parsing callers must migrate to the Source
`TypeSpelling` trait, as documented in the hgl-source card.

May use hgl-types and the existing jiff dependency; cannot depend on source AST,
parsing, semantic binding or compiler emission. Optional reporting target: 1270 lines. Dependency walls remain enforced;
numerical targets do not constrain source size.
The `ir_visit` module owns source-ordered read/mutable expression, statement,
hook and lexical-node walkers with subtree pruning. It never executes values;
helper scope policies stay with callers. `Value::closed` uses this traversal
with its constructor/literal whitelist.

DEC0004 reason 3: source, model and checked execution each exceed 1000 lines
under xtask's code-line count. Cargo prevents lower layers from reaching source
parsing and name resolution. The relocation changes no language behavior.

Native contract metadata, cold independently owned NativeValue data and checked invocation phase metadata remain in this model. Native checked IR carries exact descriptor-backed helper bindings and cold-call flags; execution happens only in hgl-eval or selected target callbacks. The model cannot depend on source parsing, semantic checking or executable providers.

`Ty::Result(T, E)` and `Ty::Error` are intrinsically atomic ordinary data, with
equality only and no key/hash/order capability. Result equality selects only the
active payload; Error equality excludes provenance. `Plan::captures: Vec<Capture>`
uses `Capture { owner, nodes, output: Option<usize>, value: Ty }`. The selected
normal type stays `value`; the capture owner's internal REF bundle is an endpoint
view, not a source REF return. `Plan::keyed: Vec<Keyed>` uses
`Keyed { owner, nodes, member }` for independent outputless keyed composition.
The synthetic member represents an exact boundary endpoint and never executes.
