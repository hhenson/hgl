# Card: hgl-rust-snapshot-slots

Own independent node-local Tuple/concrete Struct observation storage and prepared transport.
The `snapshot_slots` module has an initial 350 source-line budget within the
unchanged hgl-rust crate limit. It uses layouts, observed endpoints and snapshots.

Public surface: `prepare`, `local`, `projection`, `read`, `publish`, `publication`, `returned`, `endpoint_read`, `native_text`.
`prepare` clones the emission plan, reserves a distinct private typed destination
for each retained local, and inserts implicit locals for direct retained returns or own-output assignments or
runtime tuple construction containing observed children. Source types remain
unchanged. Private destinations are NUL-prefixed, inaccessible source entries;
their exact handles and descendant capacities bind before hooks run.

`local` captures every current held child and every live map key in prepared KeyId order, retaining
presence recursively in preallocated Optional slots. It copies another owner
with PreparedValue preflight and copy_within. Read-only locals never alias an
input or another owner. `read` projects typed positions; absent required reads
fail through the existing uncatalogued ordinary operation failure path. No
stable error code is invented for that spec gap. Publication copies present
scalar slots through prepared_global transport without owning collection
materialization. Publication delegates to structural_publication, which reconciles fixed invalid
children and exact Map membership within the pinned complete-value contract.

The bounded profile admits recursively supported scalar Tuple/Struct/fixed-List/Map
observations, with scalar map keys. Mutable retained locals, Set/enum/family/recursive/atomic descendants, aggregate
helper/native/global replacement boundaries, variable-sized text value-function boundaries, aggregate comparisons are explicitly diagnosed until their ordinary ABI
and writable-place lowering are implemented. Complete ordinary native tuples
continue to use existing ordinary helper and value-function behavior. Runtime
native collection/text child construction requires prepared observation slots.
Text projections passed to native helpers borrow their exact prepared scalar
column; embedding and constructor copies use typed copy_within, preserving
validity and capacity. An invalid embedded current Tuple fails its payload read.

Acceptance: shared three tests/eight evals and own direct return, nested embedding,
copy/read tests; the complete generated graph evaluation and recording paths
have successful first/repeated evaluations measured with CountingAllocator and
allocate zero times. Failed cycles retain lifecycle cleanup and are reported
separately; their error construction is outside the successful-tick profile. Existing adapter proof and capacity budgets are unchanged.

Owning observation adapters require the existing finite prepared execution proof;
standalone/unproved private-slot use rejects before emission. Atomic ordinary
Tuple results retain whole-value publication and cannot accept sparse snapshots.

`native_text` borrows the typed endpoint text column and checks validity for required
child reads. Direct scalar text child retention binds its input before borrowing
prepared storage, then copies into reserved local slots. Canonical source scalar
identity stays unchanged. Native complete collection children remain outside this
publication ABI; fixed-width scalar native positional results remain supported.
Map capture scans the prepared domain, then stores live entries in that order;
remove/reinsert cannot introduce live-vector ordering assumptions. Invalid keys
remain stored with unset child slots. Empty/wholly invalid Map reads fail through
the existing uncatalogued path rather than creating a new publication policy.

Retained locals carry their existing typed prepared slot and a stack presence
Boolean. snapshot_views owns observation projection/copy versus required reads;
initializing owning child lets and constructing Tuples copy exact unset state.
Private capacity/layout preparation is unchanged. Temporal-root validity errors
remain separate from value.unset_read on retained ordinary payload consumption.

native_scalar(&Value)->String borrows an owning scalar endpoint with existing required-child validity checks. native_text delegates this shared borrowing path and preserves its existing public result.

Readable byte input lets capture binding-time contents into these same private
slots. Scalar returns and own-output assignments use scalar_from_global; byte rolling
publications copy the slot
through rolling_from(None, ...), with complete arrival preflight before commit.
Local byte comparison and length borrow the slot; alias lets own separate copies.
Retained byte children can enter existing prepared Tuple construction. Direct
unprepared owning child construction and mutable retained locals stay diagnosed.
Acceptance includes measured direct/observed byte lets, scalar/rolling returns,
aliases, independent locals, tuple children, empty/equal arrivals and silence.

Byte locals initialized by delta_value on a readable rolling input capture the
latest arrival through observed's existing rolling source transport. The snapshot
owns the Bytes payload, never a whole window or a borrowed endpoint. Returns and
assignments then use the same scalar/rolling byte destination selection.
