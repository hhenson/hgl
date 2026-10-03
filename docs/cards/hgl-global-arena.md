# Card: hgl-global-arena

Reusable run-owned storage for typed ordinary values. Uses `hgl-types` and
`hgl-columns`; budget 220 lines. No unsafe code or third-party dependencies.

`Capacity` accumulates `scalar<T: Scalar>()` and `list()` reservations.
`Columns` owns eight primitive columns, matching free-slot pools, and list
entries. `reserve(&Capacity) -> NodeResult` obtains capacity before any logical
mutation, including capacity required to reclaim every allocated slot.
`insert<T>(T) -> usize`, `scalar<T>(usize) -> &T`, `replace<T>(usize,T)`, and
`release<T>(usize)` statically select scalar storage. `insert_list(ListData)`,
`list(usize)`, `list_mut(usize)`, `replace_list(usize,ListData) -> ListData`, and
`release_list(usize)` address list descriptors without type dispatch.
`ListData` is `Vec<Vec<usize>>`: each inner vector is an element's prepared typed
positions, in marker declaration order. It stores no keys or runtime type tags.

The typed value layer reserves, initializes and releases every descendant. List
replacement retains a stable root descriptor and releases old descendants to
free pools. Repeated replacements reuse slots rather than leaking columns.
Capacity and free pools are bounded by the peak live plus staged slot count;
failed reservations can increase physical capacity but cannot alter live values.
`slot_counts() -> (usize, usize)` exposes allocated scalar/list slots for tests.

No arena schema inspection, name lookup, dynamic downcast, reference counting or
borrow registry occurs in hooks. Scalar selection is compile-time specialization.
Mutants: skip reclaim capacity; never reuse freed slots; share list descriptor
contents on retention; link an element before preparation succeeds.

`Layouts::{push(ListData)->NodeResult, take()->ListData}` and `Default` hold
preallocated buffers in typed traversal order. Taking a layout moves its buffer;
there is no allocation, schema inspection or payload copy during commit.
`Capacity::lists(usize)` batches known descriptor demand; arithmetic saturates so
oversized requests reliably fail reservation before any logical value is changed.
