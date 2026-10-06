# Card: hgl-type-syntax

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `type_syntax` module of `hgl-source` (`crates/hgl-source/src/type_syntax.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Pure nested type-spelling decomposition extracted unchanged from hgl-type-shape.
No dependencies; budget 100 source lines. Expose application(&str) ->
Option<(&str, Vec<&str>)>, delta_argument(&str) -> Option<&str>, and
list_parts(&str) -> Option<(&str, Option<usize>)>. Respect balanced nesting and
existing fixed/unbounded list syntax; do not resolve names or infer types.

hgl-type-shape re-exports application and delta_argument and keeps Ty::list_parts
as a delegate. All existing callers and type identity rules remain unchanged.
