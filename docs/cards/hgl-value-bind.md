# Card: hgl-value-bind

Bind checked argument values to source signatures and resolve their concrete
types. Uses hgl-source, hgl-library, hgl-rust-ir and hgl-value-types; budget 500 source lines.

Public surface: bind, supported_type, resolve_type, method_arguments,
order_arguments, constant. This layer preserves exact ordinary list/nominal
identity, generic bindings and contextual result types. It distinguishes fixed
configuration, ordinary value-function arguments and temporal ports. Closed
ordinary list/struct data can be configuration; aggregate temporal ports remain
unsupported. It never executes values or creates runtime graph scheduling.

Acceptance: hgl-program signature selection regressions, readonly ordinary
parameters, concrete empty-list parameter context and node configuration.

`signature_types` resolves selected signature source names and infers nested
ordinary nominal/list patterns using hgl-value-types before ordinary bind checks.
Complete ambient type substitutions are retained for generic function bodies;
concrete nominal results are specialized after parameter inference.

`native` validates an exact selected native implementation, interns its checked
signature and returns Native value IR. This moves native binding preparation out
of graph checking; it neither calls providers nor broadens wiring-time effects.

Exact recursive publication shapes are admitted at temporal boundaries;
structural Delta values are not temporal shapes. Signature inference recognizes
inverse delta_of relationships recursively through ordinary lists/nominal
arguments before binding the selected operator. No payload-content inference
or result-only structural inference is introduced.

`instantiated` matches explicit materializations against fully resolved exact
source types, retaining nominal origins, phantom arguments and recursive shape.
Implementation binding uses the same nested signature inference as declarations.
