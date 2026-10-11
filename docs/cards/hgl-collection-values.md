# Card: hgl-collection-values

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `collection_values` module of `hgl-semantics` (`crates/hgl-semantics/src/collection_values.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Complete ordinary collection identity and ordered cold construction. Uses
source, checked IR, complete composite keys and family values; budget 200 lines.

`key(&Value)->Result<hgl_composite_keys::Key,String>` validates a retained complete key,
including optional presence, concrete nominal identity and rejecting NaN in any present scalar leaf. `evaluate<E:From<String>>(&Ty,&[Value], impl
FnMut(&Value)->Result<Value,E>)->Result<Vec<Value>,E>` independently retains each
key, validates and checks duplicates before evaluating its map value. Failure
returns no partial collection and never executes later expressions.

`equal(&Value,&Value)->Result<bool,EvalError>` reexports authoritative fallible
owning-value comparison, preserving provider and nested box failures. Cold
construction uses key_context for provider-equal native identities; source
checking retains the existing known-key path. The module now belongs to hgl-eval.
Acceptance: instrumented provider ordering, duplicate short circuit, once-only
materialization, nested unordered equality and NaN/signed-zero boundaries.
