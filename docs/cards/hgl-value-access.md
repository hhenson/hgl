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

Re-export hgl-static-values::StaticValues for existing callers. Provenance and
closed-key validation belong to that crate; ordinary runtime access authority
remains here. May depend on hgl-static-values.

Ordinary projection preserves struct optional-position metadata. Access to an
optional field is explicitly rejected in this publication slice; required
fields and whole-value owning bindings keep existing behavior. No optional
clearing or mutation surface follows from admitting complete snapshots.

Ordinary sets/maps are owning aggregates at existing retention and borrow boundaries. This adds no element projection or mutation syntax.
