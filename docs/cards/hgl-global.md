# Card: hgl-global

Run-owned ordinary scalar values, independent of temporal endpoints. Uses
`hgl-types` and `hgl-columns`; initial budget 200 lines. No third-party crates.

`GlobalState` is initially unprovisioned. `provision()` enables one run's
storage. `bind<T>(key)` and `prepare(key, ScalarType)` reconcile keys and exact
types during construction. Equal keys share one entry; conflicting types or
an unprovisioned request fail. Preparation leaves the value absent.

`Global<T>` is an opaque typed, Copy handle even for String. `get(handle)` and
`set(handle, &value)` use typed columns and indexed presence, with no key lookup
or runtime type tests. A missing get reports its key. Set copies successfully
before replacing the earlier value. Strings use fallible owned copies; no
allocation-free claim applies to them. Handles belong to their owning state,
as Store endpoint handles do; mixing owners is a Rust caller error.

There are no graph roles, scheduling, endpoint publication, sequence operations,
process-wide registry, or aggregate access semantics. The run owner handles
configuration/extraction; nodes receive only prepared handles.

Acceptance: all eight scalars; absent versus present false/zero/empty text;
same-key coalescing, conflict errors, independent runs, owned text after
replacement/disposal; pre-start/nested-template integration in hgl-describe.
