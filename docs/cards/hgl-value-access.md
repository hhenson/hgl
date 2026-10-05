# Card: hgl-value-access

Ordinary access classification, separate from effect traversal and payload
operations. Dependencies hgl-source/hgl-rust-ir; budget 200 source lines.

Public: ordinary, writable, field, provenance, binding, helper_argument,
observed, `project(&Ty) -> Ty`. Projection recursively erases atomic field
boundaries in ordinary payloads and containers, preserving nominal generic
arguments and delta origins. Temporal schemas retain their field boundaries;
projection does not broaden ordinary type admission.
Owning values, lexical global aggregates and evaluation-local
structural delta observations retain distinct provenance without source type
qualifiers. Delta objects admit ownership and whole replacement, no inspection.
An immutable observation alias preserves observation identity; an annotation
adds no ownership and a var initializer cannot upgrade it. Helper boundaries
reject observations and lexical borrows; explicit retention operations consume
independent copies. Scalar delta_value remains owned. Existing struct/list
projection and lexical borrow authority rules are unchanged.

Acceptance: source borrowed global tests plus structural delta observation,
retention, typed alias and helper-escape checking tests.

## Static preparation provenance

Compiler-only `StaticValues` derives Default and exposes `prepared: Vec<Value>`,
`configuration: Vec<Value>`, and `locals: BTreeMap<usize, Value>`.
`resolve<'a>(&'a self, value: &'a Value) -> &'a Value` follows known cold binding
provenance. `arguments(&self, signature: &hgl_library::Signature, args: &[Value],
positions: &[usize]) -> BTreeMap<usize, Value>` retains static constant-parameter
metadata under the source-order lexical argument mapping. May use hgl-library.
Neither method evaluates an expression, fabricates data, or invokes a provider.
Unresolved local values remain unresolved. Existing runtime access authority is
unchanged; this data is never installed in the runtime store.
