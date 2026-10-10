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
