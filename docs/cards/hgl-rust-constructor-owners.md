# Card: hgl-rust-constructor-owners

The constructor_owners emitter reserves complete canonical destinations for private ordinary constructor locals. It may use prepared_hooks, layouts and constructor_fields; initial budget 180 lines within the unchanged Rust crate cap.

Internal surface: local, global_bounds, fields, family and origin. Checked finite constructor structure supplies recursive child bounds without executing operands during preparation. Fields write in supplied source order, while typed slots preserve declaration order. Optional and recursive child storage is preflighted and retains exact presence. Whole boxing and ordinary projections share the retained owner through static slot metadata.

Acceptance: measured successful recursive constructor ownership, source-order provider calls with once-only counts, and independently retained Any field values, in debug and optimized profiles. No evaluation allocation, runtime name lookup or dynamic type test is introduced.

Concrete family constructors select the static discriminator and typed member slot. Copies preserve the checked finite origin for preparation and boxing, while each copy owns an independent destination.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.


The private source selector and staging emitter retain constructor fields in
written order, releasing storage borrows before evaluating each next operand.
Staging uses a distinct reserved owner; a failed field retention cannot expose a
partial replacement of the caller's destination. May also use constructor_fields.

Field writing and schema are now owned by constructor_fields. The canonical
owner emitter exposes local, source, global_bounds, family and origin; source
retains into its private owner before returning a checked typed slot.
Native mutable locals, assignments and growing-owner exclusions remain intact.

Canonical local-owner queries carry both value and bound origin; private
expression stages carry one value. Ordinary growing locals read the completed
stage into their existing ordinary representation rather than inheriting a
canonical slot identity. Existing list-growth source controls cover this boundary.

Public destination emits the selected typed private slot handle without executing its initializer. Public source performs ordered staging and returns that typed handle. The executable generator assembler uses those emitter boundaries; resume handles alone never reexecute a constructor.
