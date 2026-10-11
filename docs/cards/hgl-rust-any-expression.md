# Card: hgl-rust-any-expression

The any_expression emitter compiles ordinary constructor trees into frames written directly to reserved Any buffers. It may use any_values and source_slots; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: encoding, compose and limits. Leaves retain their static typed source or evaluate once before mutation. Complete constructor fields, optional presence and recursive child constructors require no intermediate native lists or boxes. Width planning counts checked expression structure and ordinary cold leaf bounds without executing providers or hooks. Nested Any constructors flatten at the box boundary and retain a descriptor within an ordinary field.

Acceptance: generated recursive constructor with owning list and present child, measured after preparation; retained boxed sources and compound mutable box replacement remain independent.

Constructor operands are cached once in supplied source order before encoding in declaration order. Private canonical constructor owners contribute their checked recursive bounds to width planning.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.

The any_expression_leaves sibling owns typed leaf source selection (initial ceiling 50, uses layouts, any_values and source_slots). Both plain frames and nested box leaves share its codec selection. Scalar input views validate before writing and borrow immutable observation storage during measurement/copy without allocating a String. The parent module uses this sibling and retains its existing 180-line ceiling.

Named-operand controls also verify once-only written order and failure stopping
later operands in the descriptor-stage path.
