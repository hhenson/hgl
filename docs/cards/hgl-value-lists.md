# Card: hgl-value-lists

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_lists` module of `hgl-store` (`crates/hgl-store/src/value_lists.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Stable ordinary list descriptors, depending on hgl-types; budget 120 source lines.
No unsafe code or third-party dependencies. `ListData = Vec<Vec<usize>>` contains
only marker-selected descendant positions, never payload type tags or values.

`Lists: Default + Debug` owns descriptor entries and reusable indices.
`reserve(count)->NodeResult` reserves insertion and reclamation capacity;
`insert(ListData)->usize`, `get(slot)->&[Vec<usize>]`,
`get_mut(slot)->&mut ListData`, `replace(slot,ListData)->ListData`,
`release(slot)`, `len()` and `is_empty()` preserve dynamic list storage semantics.
Dynamic mutation of a prepared descriptor is a caller error. Replacement returns
all physical descendants, including inactive ones, for typed reclamation.

`prepared(slot)->&[Vec<usize>]` exposes retained positions and
`set_len(slot,length)` publishes a preflighted active length without allocation.
Inactive positions remain owned and reusable but invisible to ordinary list reads.
Only the typed value layer interprets these positions or copies payloads.
Acceptance: shrink/empty/reinsert without capacity loss, no phantom elements,
stable independent captures and reclamation of every retained descendant.
