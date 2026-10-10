# Card: hgl-rust-any-expression

The any_expression emitter compiles ordinary constructor trees into frames written directly to reserved Any buffers. It may use any_values and source_slots; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: encoding, compose and limits. Leaves retain their static typed source or evaluate once before mutation. Complete constructor fields, optional presence and recursive child constructors require no intermediate native lists or boxes. Width planning counts checked expression structure and ordinary cold leaf bounds without executing providers or hooks. Nested Any constructors flatten at the box boundary and retain a descriptor within an ordinary field.

Acceptance: generated recursive constructor with owning list and present child, measured after preparation; retained boxed sources and compound mutable box replacement remain independent.
