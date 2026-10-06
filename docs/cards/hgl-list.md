# Card: hgl-list

Ordinary owning and globally borrowed list operations under LIST-EMPTY/READ/GROW/
RETAIN/ERROR, VAL-17, and value-mutability. Uses `hgl-types` and
`hgl-global-value`; budget 260 lines. No unsafe code or third-party dependencies.

`List<T: GlobalValue, const N: i64 = -1>` is a marker with owning Value
`Vec<T::Value>` and one prepared list descriptor slot. -1 denotes unbounded;
nonnegative N denotes exact fixed length. Its exact OrdinaryType carries the
recursive element type and optional fixed length. Retain and preparation validate
host-provided lengths, including nested lists, before any live mutation.

`list_len(&[T]) -> NodeResult<i64>`, `list_index(&[T],i64) -> NodeResult<&T>`,
`list_index_mut(&mut [T],i64) -> NodeResult<&mut T>`, and
`list_push<T: GlobalValue>(&mut Vec<T::Value>, &T::Value) -> NodeResult` serve
ordinary evaluation. Mutable index is an internal projection helper, never a new
source indexed replacement operation. Push retains the item and obtains capacity
before extending; the compiler evaluates self-source items before mutable access.

`global_len<T,N>(&ValueColumns, ValueSlot<List<T,N>>)`,
`global_index<T,N>(&ValueColumns, ValueSlot<List<T,N>>,i64)` and
`global_push<T>(&mut ValueColumns, ValueSlot<List<T>>, &T::Value)` provide typed
prepared accesses. Index returns ValueSlot<T>, reconstructing positions only,
without reading/copying payloads. Push is available only for unbounded markers.
It prepares all recursive layouts, reserves arena capacity and receiver capacity,
installs retained elements, then links the final element. Physical reservation may
change capacity on failure; length and recursively observed values stay unchanged.
List replacement retains a stable root slot and releases all old descendants with
compile-time marker operations into reusable pools. No per-hook schema lookup,
type dispatch, borrow registry, whole-list copy for access or push, or scheduling.

Acceptance: exact fixed/unbounded identities and seed lengths; negative/end/empty
bounds; all eight primitive elements; nested struct/list retention independence;
self-source push; writable nested projections; failed retention/capacity atomicity;
no copies/allocations at borrow,len,index; bounded arena high-water under repeated
replacement and errors. Fixed growth is rejected by source checking and the typed
runtime push API. Positive fixed construction requires exactly N existing values.

The List marker and its owning/finite-prepared storage implementations now live
in hgl-list-storage and are reexported unchanged. hgl-list keeps ordinary list
operations and additionally reexports ListBounds, append_slot and commit_append.
Finite prepared copying retains vacant descendants; dynamic operations remain
explicitly outside that protocol.
