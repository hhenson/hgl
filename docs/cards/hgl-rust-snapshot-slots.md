# Card: hgl-rust-snapshot-slots

Own independent node-local Tuple observation storage and prepared transport.
The `snapshot_slots` module has an initial 350 source-line budget within the
unchanged hgl-rust crate limit. It uses layouts, observed endpoints and snapshots.

Public surface: `prepare`, `local`, `projection`, `read`, `publish`, `returned`, `endpoint_read`.
`prepare` clones the emission plan, reserves a distinct private typed destination
for each retained local, and inserts implicit locals for direct tuple returns or
runtime tuple construction containing observed children. Source types remain
unchanged. Private destinations are NUL-prefixed, inaccessible source entries;
their exact handles and descendant capacities bind before hooks run.

`local` captures every current held child and every live map key, retaining
presence recursively in preallocated Optional slots. It copies another owner
with PreparedValue preflight and copy_within. Read-only locals never alias an
input or another owner. `read` projects typed positions; absent required reads
fail through the existing uncatalogued ordinary operation failure path. No
stable error code is invented for that spec gap. Publication copies present
scalar slots through prepared_global transport without owning collection
materialization. Missing children and absent map keys retain the existing
publication-profile exclusions described by hgl-rust-snapshots.

The bounded profile admits recursively supported scalar Tuple/List/Map
observations, with scalar map keys. Mutable retained locals, Set/nominal descendants, aggregate
helper/native/global replacement boundaries, variable-sized text value-function boundaries, aggregate comparisons and nominal
containers of retained tuples are explicitly diagnosed until their ordinary ABI
and writable-place lowering are implemented. Complete ordinary native tuples
continue to use existing ordinary helper and value-function behavior. Runtime
native collection/text child construction requires prepared observation slots.
Text projections passed to native helpers borrow their exact prepared scalar
column; embedding and constructor copies use typed copy_within, preserving
validity and capacity. An invalid embedded current Tuple fails its payload read.

Acceptance: shared three tests/eight evals and own direct return, nested embedding,
copy/read tests; the complete generated graph evaluation and recording paths
are measured with CountingAllocator and allocate zero times in first and repeated
evaluations. Existing adapter proof and capacity budgets are unchanged.

Owning observation adapters require the existing finite prepared execution proof;
standalone/unproved private-slot use rejects before emission. Atomic ordinary
Tuple results retain whole-value publication and cannot accept sparse snapshots.
