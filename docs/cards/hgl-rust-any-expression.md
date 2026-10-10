# Card: hgl-rust-any-expression

The any_expression emitter compiles ordinary constructor trees into frames written directly to reserved Any buffers. It may use any_values, source_slots and observed; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: encoding, compose and limits. Leaves retain their static typed source or evaluate once before mutation. Complete constructor fields, optional presence and recursive child constructors require no intermediate native lists or boxes. Width planning counts checked expression structure and ordinary cold leaf bounds without executing providers or hooks. Nested Any constructors flatten at the box boundary and retain a descriptor within an ordinary field.

Acceptance: generated recursive constructor with owning list and present child, measured after preparation; retained boxed sources and compound mutable box replacement remain independent.

Named constructor operands prepare in source order before fields are emitted in
layout order. Each operand runs once; failure prevents later operands.

Computed text uses the shared immutable text-fragment planner: input and cold
configuration leaves (including field/index projections) are validated in written
operand order, then their bytes
are encoded directly into the reserved box frame. No intermediate String is
constructed. Text bounds are widened before ordinary and constructor Any widths
are calculated, including concatenations beneath boxing and family wrappers.
Private canonical constructor owners contribute their checked recursive bounds
to width planning.
