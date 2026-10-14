# Card: hgl-rust-collections

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `collections` module of `hgl-rust` (`crates/hgl-rust/src/collections.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Checked ordinary collection layout, constructor and equality emission. Uses
source, IR and prepared-value emission; budget 250 lines, no runtime execution.

`element(&Ty)->Option<Ty>` selects a list/set item or exact map pair.
`marker(&Ty, global:fn(&Ty)->String, schema:fn(&Ty)->String)->String` emits
an exact Set/Map schema marker delegating physical owning retention and prepared
copies to List storage of the selected item. No type-erased runtime lookup.

`construct(&Ty,&[Value],emit:impl Fn(&Value)->String,owned:fn(&Ty)->String)` emits
source-ordered native construction with immediate key validation and duplicate
checking before each map value. `equal(&Ty,&str,&str,fn(&Ty)->String)->String` recursively emits
unordered collection comparison and ordered descendant comparison.
Acceptance: generated debug/release shared snapshots and complete nested
retention, equal empty publications, replay/record and sparse atomic children.

retained(&str,&Ty,fn(&Ty)->String)->String emits independent owning reads through exact composite markers and the existing scalar copy contract. The layout callback selects the concrete marker without runtime type tests.

Constructor key validation walks finite tuple/concrete struct fields, preserving optional presence and rejecting every present NaN leaf before the associated map value executes. Native key equality uses complete exact typed components; keys cannot contain collections, recursive values or abstract families.

Native equality preserves optional field presence and family discriminators. Recursive descendants invoke generated typed marker comparison methods, so recursion follows only finite present payloads and never allocates or expands schemas during emission.

family_field maps only declared family/common field identities through each retained member schema. Its slot flag selects either native payload projection or a canonical typed slot: canonical parents evaluate once, read their retained tag, and return the active member's declared field slot without materializing the owning family. iteration_start traverses retained map/list membership or modified keys without copying keys; missing removed children are skipped for the modified selector. Child locals retain exact endpoint types and scalar payload reads use typed transport.

Fixed-list items traversal selects declared dense child projections independently
of keyed membership. Its modified filter reads each child's endpoint modification
metadata; full traversal includes unset children for metadata inspection.

Cold field equality and key validation share canonical ordinary fields/optional
metadata with box planning. Scalar retention uses the shared owning_scalar profile
while preserving the existing Any ownership boundary.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.

Point-in-time dictionary lookup hashes exact compiled keys and probes reusable owner/descriptor indexes. slot_key_equal borrows scalar, Native and composite key descendants only for collision equality. Written fallback operands execute once before lookup selection. Missing strict reads originate value.missing_key; safe reads preserve present zero/false/empty values independently of absence. Owning result extraction retains its ordinary ownership boundary.

Complete ordinary map owners retain cached hash metadata. Compiled key_hash uses existing exact primitive, Native, Any and composite key capabilities. Prepared map markers provision and copy an independent index with their original descriptor layout; ordinary ownership boundaries retain entries and index together.
