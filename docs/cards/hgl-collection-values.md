# Card: hgl-collection-values

Complete ordinary collection identity and ordered cold construction. Uses
source, checked IR, scalar keys and family values; budget 200 lines.

`key(&Value)->Result<hgl_scalar_keys::Key,String>` validates a retained scalar,
including rejecting NaN. `evaluate<E:From<String>>(&Ty,&[Value], impl
FnMut(&Value)->Result<Value,E>)->Result<Vec<Value>,E>` independently retains each
key, validates and checks duplicates before evaluating its map value. Failure
returns no partial collection and never executes later expressions.

`equal(&Value,&Value)->bool` recursively compares exact ordinary types and
concrete family members, with unordered Set/Map entries and ordered lists.
Acceptance: instrumented provider ordering, duplicate short circuit, once-only
materialization, nested unordered equality and NaN/signed-zero boundaries.
