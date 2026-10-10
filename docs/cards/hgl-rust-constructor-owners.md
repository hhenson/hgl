# Card: hgl-rust-constructor-owners

The constructor_owners emitter reserves complete canonical destinations for private ordinary constructor locals. It may use prepared_hooks and layouts; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: local and global_bounds. Checked finite constructor structure supplies recursive child bounds without executing operands during preparation. Constructor writes delegate through prepared_hooks to constructor_fields, preserving source order and optional presence. Optional and recursive child storage is preflighted. Whole boxing and ordinary projections share the retained owner through static slot metadata.

Acceptance: measured successful recursive constructor ownership, source-order provider calls with once-only counts, and independently retained Any field values, in debug and optimized profiles. No evaluation allocation, runtime name lookup or dynamic type test is introduced.
