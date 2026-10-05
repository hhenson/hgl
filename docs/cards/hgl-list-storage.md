# Card: hgl-list-storage

Ordinary list marker storage, using hgl-global-value, hgl-prepared-value and
hgl-types; budget 300 source lines. No unsafe code or third-party dependencies.
The public hgl-list::List name remains a reexport of this marker.

`List<T:GlobalValue,const N:i64=-1>` implements the existing owning GlobalValue
protocol unchanged: Value is Vec<T::Value>, Slots is one descriptor index,
-1 means unbounded and nonnegative N is exact length. Dynamic retention,
preparation, installation, replacement and release retain independent descendants.
Required fixed length is validated before mutation.

For T:PreparedValue it also implements PreparedValue.
`ListBounds<T>{pub len:usize,pub element:T::Bounds}:Default+Debug` merges the
largest finite length and element capacities. Allocation creates separate storage
for every possible element. Active length starts zero; fixed length is enforced
when copying a published value. Shrinking and empty copies retain vacant element
slots and their recursively reserved capacities. Reads see active elements only.
Whole-list checks traverse all source elements before any destination changes.
Copies use typed descendant slots, never temporary owning payload/layout vectors.

`append_slot<T>(&ValueColumns,ValueSlot<List<T>>)->NodeResult<ValueSlot<T>>`
returns the next reserved independent element without altering logical length.
`commit_append<T>(&mut ValueColumns,ValueSlot<List<T>>)` publishes one element
only after complete preflight and copy. A rejected append leaves length and all
active records unchanged. Extraction through GlobalValue::read is an explicit
owning boundary outside evaluation and includes only active elements.

Acceptance lives in hgl-store/tests/prepared_values.rs alongside all unchanged
ordinary list regressions. It measures first/repeated/empty/larger publication,
atomic pass, record append and independent retained captures.
