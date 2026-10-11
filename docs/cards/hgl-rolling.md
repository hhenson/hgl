# Card: hgl-rolling

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `rolling` module of `hgl-store` (`crates/hgl-store/src/rolling.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Prepared rolling arrival retention under normative rolling-publications a3f5cf8.
Uses bindings, global-value, prepared-value, shapes and types; budget 350 source
lines. No unsafe code or third-party dependencies.

`WindowShape` fixes Payload, DURATION, MAX and MIN statically. The typed marker
`Rolling<T,DURATION,MAX,MIN>` implements Shape and WindowShape. `Arena` owns
independent ordinary columns and endpoint rings. `add_output` creates an invalid
root; `prepare_output<S>` cold-allocates complete independent payload slots from
finite bounds and arrival horizon. Tick windows reserve min(Max,horizon) slots;
duration windows reserve horizon slots. No defaults become visible.

`borrow<S>` returns the latest arrival slot, `values` exposes its immutable
columns, and `ready<S>` inspects current retained count/span without eviction.
`write`, `from` and `pass<S,T,W>` preflight the whole payload into a reserved destination,
copy independently, then commit the arrival and publish. For `pass`, S and T
have the same Payload; retention and readiness follow T’s window policy.
The prepared Store facade
validates writer/time/scope/generation before calling these operations. Failed
fits leave membership, readiness and publication metadata unchanged. Invalidation
resets logical readiness; the next arrival starts a fresh window.

Only a new arrival evicts: full tick windows remove the oldest; duration windows
remove entries older than Max (equality is retained). Duration Min=0 is ready on
the first arrival. Silence never evicts or schedules work. Forwarded arrivals
enter the output window at the output publication time. Static generic policy
selects behavior without runtime schema dispatch or hot allocation.

Acceptance: native zero-allocation publication/pass tests cover inclusive age,
idle readiness, delayed independent output, failed fit, repeated ring reuse,
owning strings, invalidation and reactivation. Generated shared six-case tests
count every Graph::evaluate call in debug/release, including first publication,
replay, pass-through and prepared recording. No claim extends to arbitrary
unbounded dynamic native sources outside the finite prepared profile.

text<S> prepares a String arrival destination, checks complete measured byte
capacity, then invokes an infallible generated composition callback before
committing the ring arrival. The reserved buffer is moved temporarily, retaining
capacity, and the callback observes independent source storage. Failed capacity
checks leave the prior window and publication metadata unchanged.

`bytes<S,W>(bindings,output,(now,wake),copy)` supplies a reserved byte arrival
destination to a callback that completes all fallible preflight before mutation.
Only a successful copy commits the arrival. Atomic octet conversion uses this
boundary; invalid octets or insufficient capacity preserve the prior arrival,
readiness and publication stamp, including reuse of a full one-slot ring.
