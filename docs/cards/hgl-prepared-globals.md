# Card: hgl-prepared-globals

Copy typed ordinary scalar positions into prepared temporal destinations.
The `prepared_globals` module has an initial 80 source-line budget within the
unchanged hgl-store crate limit. It extends PreparedTick and uses scalar columns.

Public surface: PreparedTick::scalar_from_global. The source is a compiler-selected
prepared scalar position; the target is an exact output identity and generation.
Existing authorization checks owner, lifetime, phase and scope. GlobalState's
typed copy_scalar checks complete source capacity before copying and publication.
The operation never binds names, inspects source kinds, allocates, fabricates a
missing value or changes ordinary root presence. Source validity is checked by
the caller's typed optional projection before this operation.

Acceptance: generated Tuple observations and recording under zero per-tick
allocation measurement, including actual false and initially absent children.

PreparedTick::atomic_from<T:PreparedValue>(&ValueColumns, ValueSlot<T>, Output<Atomic<T>>)->NodeResult publishes from an independent prepared ordinary arena after complete preflight. May use global and shapes. This shares the same source-storage boundary as scalar_from_global.

PreparedTick::rolling_from<S:WindowShape>(Option<&ValueColumns>,ValueSlot<S::Payload>,
Output<S>)->NodeResult uses the same independent prepared-source boundary for
rolling arrivals. It delegates complete validation and publication to the
existing rolling arena. Some borrows an external source arena; None selects the
store's independently owned global slots without overlapping Store borrows. Exact
source slot shape and output authority remain checked. May use rolling; no endpoint or payload is reinterpreted.

`rolling_bytes_from_list<S,const N>(Input<Atomic<List<i64,N>>>,Output<S>)` requires
WindowShape with bytes payload. It authorizes the output and uses atomic octet
preflight to copy directly into a reserved rolling byte arrival. May use list.
