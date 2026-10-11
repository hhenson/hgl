# Card: hgl-rust-any-comparisons

The Any comparison emitter reserves two private box operands using existing
prepared globals and codecs. Operands execute once, left to right, and a failed
left operand prevents evaluation of the right. Recursive preparation covers
nested expressions without native allocating box temporaries. Comparison
capability and error behavior remain descriptor operations.

The module may use any_prepared, any_values and values; budget 100 lines inside
the unchanged hgl-rust crate cap. No dependency on ordinary family owner
comparison or default-constructor features is required.
