# Card: hgl-rust-source-slots

Typed immutable configuration projections for emitted source nodes. Uses
hgl-rust-ir and hgl-rust-layouts; budget 100 source lines. No runtime execution,
third-party dependencies or source admission rules.

`projection(&Value,emit_index:impl Fn(&Value)->String)->Option<String>` emits a
typed ValueSlot expression only for paths rooted at Kind::Configuration.
Configuration indexes name self.configuration_slotN; Field selects the already
typed child slot; Index bounds-checks an active list descriptor in
self.configuration_columns and reconstructs its element marker positions.
Parent and index evaluate once, in that order. Unsupported roots return None and
preserve the caller's ordinary expression path.

The node emitter independently prepares its immutable configuration_columns and
configuration_slotN fields before start. Slots never reference writable source
locals, caller-owned buffers or mutable graph input storage. They are safe to
retain across generator suspension because configuration lifetime equals the node.
No owning payload extraction, source re-evaluation, dynamic type lookup or layout
allocation occurs during projection.

Acceptance: hgl-rust-generators generated debug/release execution, parked payload
slot resumption with zero allocations around each complete Graph::evaluate, and
mixed prepared/ordinary yields. Full shared replay input/record integration remains
the compiler's independent end-to-end gate.

initialize(configurations:&[Value],prepared:bool,node_name:&str,
emit:impl Fn(&Value)->String)->String emits cold configuration-column creation.
When arguments were already materialized, it reuses them; otherwise each checked
native expression evaluates once. For each independent configuration root it
merges bounds, allocates all descendants, checks complete native shape/capacity,
then copies into typed storage. Any failure becomes InvalidNodeType before hooks
start. Native owning configurations remain available for existing condition logic.
The generated names are configuration_columns and configuration_slotN, matching
projection's source lifetime contract.
