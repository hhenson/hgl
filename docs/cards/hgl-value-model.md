# Card: hgl-value-model

Canonical ordinary metadata and contextual scalar materialization below source
parsing and checking. Public modules preserve the existing diagnostics, literals,
nominal_batch, ordinary_shape, scalar_shape, time_context, type_shape and
type_syntax contracts. Root exports retain Issue, literal forms, Ty, nominal and
window metadata. hgl-source reexports the same public paths.

May use hgl-types and the existing jiff dependency; cannot depend on source syntax,
semantic binding or compiler emission. Budget 1200 with unchanged moved module
ceilings. Existing provider tests move with their implementation; source clients
continue testing the facade.

DEC0004 reason 3: the ordinary model and the upper parsing/checking layer each
exceed 1000 lines. Cargo prevents the model from reaching source syntax and name
resolution. No runtime or language behavior changes at this boundary.
