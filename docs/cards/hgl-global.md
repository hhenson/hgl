# Card: hgl-global

Run-owned ordinary values, independent of temporal endpoints. Uses
`hgl-types`, `hgl-global-value` and `hgl-list`; initial budget 200 lines. No third-party crates.

`GlobalState` is initially unprovisioned. `provision()` enables one run's
storage. `bind<T>(key)` and `prepare(key, OrdinaryType)` reconcile keys and exact
types (including nominal identity and required fields) during construction. Equal keys share one entry; conflicting types or
an unprovisioned request fail. Preparation leaves the value absent.

`Global<T>` is an opaque typed, Copy handle even for String. `get(handle)` and
`set(handle, &value)` use typed columns and indexed presence, with no key lookup
or runtime type tests. A missing get reports its key. Set copies successfully
before replacing the earlier value. Strings use fallible owned copies; no
allocation-free claim applies to them. Handles belong to their owning state,
as Store endpoint handles do; mixing owners is a Rust caller error.

There are no graph roles, scheduling, endpoint publication, sequence operations,
process-wide registry, or temporal collection operations. The run owner handles
configuration/extraction; nodes receive only prepared handles.

Acceptance: all eight scalars; absent versus present false/zero/empty text;
same-key coalescing, conflict errors, independent runs, owned text after
replacement/disposal; pre-start/nested-template integration in hgl-describe.

`Global<T: GlobalValue>` carries one presence entry and typed prepared slots.
`borrow(handle)` checks presence and returns `ValueSlot<T>` without copying any
payload. `read(slot)` obtains owning data only at explicit retention boundaries;
`write(slot, &value)` retains the complete replacement before infallible leaf moves.
`get`/`set` preserve the scalar API and additionally support explicit aggregate
owner extraction/retention. Projection never initializes an absent entry. A single
root presence covers every required descendant; leaves have no string keys.
`GlobalValue`, `ValueSlot`, `ValueColumns`, `Capacity`, `Layouts`, `List`,
and ordinary `list_len`, `list_index`, `list_index_mut`, `list_push` helpers are re-exported for Store integration.

`list_len<T,N>(ValueSlot<List<T,N>>) -> NodeResult<i64>`,
`list_index<T,N>(ValueSlot<List<T,N>>, i64) -> NodeResult<ValueSlot<T>>`, and
`list_push<T>(ValueSlot<List<T>>, &T::Value) -> NodeResult` operate through prepared
list borrows. `slot_counts()` exposes arena high-water positions for tests.
Writes retain the complete value, prepare every recursive layout, and reserve
arena capacity before infallible commit. Failure preserves all logical fields and
presence; incidental physical capacity growth is permitted. Fixed host-supplied
payload lengths are checked recursively before commit. Replacement reclaims old
list descendants; repeated writes do not leak positions.

prepare_value<T:PreparedValue>(key,&T::Bounds)->NodeResult installs independent
finite capacities before any handle is bound. A present entry is rejected without
mutation. Callers must finish preparation before bind; relocating later would
invalidate existing typed projections. destination(handle)->ValueSlot<T> retrieves
prepared positions even while absent. values()/values_mut() expose typed arenas;
mark_present(handle) establishes ordinary presence only after a successful write.
PreparedValue, ListBounds, append_slot and commit_append are reexported.

Optional<T> is reexported from hgl-optional for internal field-presence storage
through the ordinary global-value facade.

Reexports Recursive and RecursiveTarget from hgl-recursive-value for generated
internal owning recursive fields. No ordinary source operation is added.
