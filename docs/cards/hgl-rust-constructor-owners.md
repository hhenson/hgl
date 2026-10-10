# Card: hgl-rust-constructor-owners

The constructor_owners emitter reserves complete canonical destinations for private ordinary constructor locals. It may use prepared_hooks and layouts; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: local, global_bounds, fields, family and origin. Checked finite constructor structure supplies recursive child bounds without executing operands during preparation. Fields write in supplied source order, while typed slots preserve declaration order. Optional and recursive child storage is preflighted and retains exact presence. Whole boxing and ordinary projections share the retained owner through static slot metadata.

Acceptance: measured successful recursive constructor ownership, source-order provider calls with once-only counts, and independently retained Any field values, in debug and optimized profiles. No evaluation allocation, runtime name lookup or dynamic type test is introduced.

Concrete family constructors select the static discriminator and typed member slot. Copies preserve the checked finite origin for preparation and boxing, while each copy owns an independent destination.
