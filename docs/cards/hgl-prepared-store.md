# Card: hgl-prepared-store

Disjoint prepared runtime access, using hgl-global, hgl-atomic, hgl-bindings,
hgl-columns, hgl-keys, hgl-shapes and hgl-types; budget 280. No unsafe code or
third-party dependencies. This facade owns no values or endpoint identity.

`PreparedStorage<'a>` borrows public columns, bindings, globals and atomic arena
mutably, plus immutable keys. `prepare_scalar<T>(OutputId,bytes)->NodeResult`
reserves a scalar endpoint's owning bytes during cold setup, including prebuilt
structural children. Atomic preparation uses the borrowed arena's prepare_output.
`observations()->(Observation,&mut GlobalState)` separates immutable temporal
sources from independently writable ordinary record destinations. Observation
exposes columns, bindings, atomic and keys; `scalar<T>(InputId)->NodeResult<&T>`
validates presence without an owning copy. Field types are compiler selected.

`tick(now,writer,&mut Wake)->PreparedTick` binds publication authority.
PreparedTick exposes storage plus these NodeResult operations:

- `scalar<T>(OutputId,generation,&T)` and
  `pass_scalar<T>(InputId,OutputId,generation)` reuse typed scalar capacity.
- `atomic<T>(Output<Atomic<T>>,&T::Value)`,
  `pass_atomic<T>(Input<Atomic<T>>,Output<Atomic<T>>)`, and
  `atomic_from<T>(&ValueColumns,ValueSlot<T>,Output<Atomic<T>>)` copy complete
  independent ordinary payloads into their preallocated destination slots.

Each operation validates lifetime, owner, scope and time, then preflights the
complete value, copies without allocation and publishes through existing Bindings.
`validate_write(&Bindings,OutputId,generation,now,writer)` is the shared authority
check also used by the legacy scalar Store::set path; diagnostics remain unchanged.
Failures never publish or partially change a held value. Prepared setup does not
publish defaults. Finite capacities are compiler responsibilities, established
before graph start. Arbitrary dynamic native values retain the legacy API.

Acceptance: unchanged Store lifetime/notification tests plus first/repeated
prepared publication, direct pass and global capture allocation/isolation tests.
Full generated graph.evaluate allocation evidence belongs to compiler integration;
native passing tests alone are not a claim about every generated hook.

`PreparedTick::text` composes a checked pure text expression directly into reserved
scalar storage. A generated measurement closure validates all sources and full
byte length before an infallible append closure writes the independent destination.
The destination is temporarily moved out of its column to permit safe disjoint
source borrowing, then restored before its single publication.
Rolling storage is a disjoint Arena field in PreparedStorage and Observation.
PreparedTick adds rolling<S>(Output<S>,&Payload), rolling_from<S>(columns,slot,
Output<S>) and pass_rolling<S>(Input<S>,Output<S>) for WindowShape markers with
PreparedValue payloads. They share the existing write authority check and only
publish after the independent complete arrival copy succeeds.

Observation is reexported from hgl-observation, preserving its existing public
surface. rolling_text<S> authorizes a String window output, measures pure scalar
source fragments through Observation, then composes into its independent reserved
ring slot and commits one arrival. The callback contract matches scalar text:
read inputs and append exactly the measured bytes, with no fallible side effects.
