# Card: hgl-ordinary-shape

The ordinary_shape module of hgl-source owns static ordinary shape metadata. It uses type_shape; initial budget 120 lines within the unchanged source crate cap.

Public surface: capable, fields and contains_any. Capability recursion follows declared child types even for empty values. fields preserves declaration order and optional positions; recursive and family definitions retain exact canonical identity. These helpers perform no evaluation, provider loading or runtime type lookup.

Acceptance: existing Any same-type capability, recursive key and concrete-family checks continue through the semantics reexports. Bool ordering belongs to its separate capability stage.
