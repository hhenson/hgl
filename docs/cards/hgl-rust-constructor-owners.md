# Card: hgl-rust-constructor-owners

The constructor_owners emitter reserves complete canonical destinations for private ordinary constructor locals. It may use prepared_hooks, layouts and constructor_fields; optional reporting target 180 lines.

Internal surface: local, global_bounds, family and origin. Checked finite constructor structure supplies recursive child bounds without executing operands during preparation. Constructor writes delegate through prepared_hooks to constructor_fields, preserving source order and optional presence. Optional and recursive child storage is preflighted. Whole boxing and ordinary projections share the retained owner through static slot metadata.

Acceptance: measured successful recursive constructor ownership, source-order provider calls with once-only counts, and independently retained Any field values, in debug and optimized profiles. No evaluation allocation, runtime name lookup or dynamic type test is introduced.

The private source selector and staging emitter retain constructor fields in
written order, releasing storage borrows before evaluating each next operand.
Staging uses a distinct reserved owner; a failed field retention cannot expose a
partial replacement of the caller's destination. May also use constructor_fields.

Concrete family constructors select the static discriminator and typed member slot. Copies preserve the checked finite origin for preparation and boxing, while each copy owns an independent destination.
