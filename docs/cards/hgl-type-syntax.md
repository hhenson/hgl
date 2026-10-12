# Card: hgl-type-syntax

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `type_syntax` module of `hgl-source` (`crates/hgl-source/src/type_syntax.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Pure nested type-spelling decomposition extracted unchanged from hgl-type-shape.
No dependencies; budget 100 source lines. Expose application(&str) ->
Option<(&str, Vec<&str>)>, delta_argument(&str) -> Option<&str>, and
list_parts(&str) -> Option<(&str, Option<usize>)>. Respect balanced nesting and
existing fixed/unbounded list syntax; do not resolve names or infer types.

The hgl-source type_shape facade reexports application and delta_argument;
TypeSpelling provides Ty::parse and Ty::list_parts above the model boundary. All existing callers and type identity rules remain unchanged.
