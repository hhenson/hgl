# Card: hgl-atomic

Prepared ordinary payload storage for atomic endpoints. Uses hgl-types,
hgl-bindings, hgl-shapes and hgl-global-value; budget 180 source lines. No unsafe code or
third-party dependencies. This reuses the ordinary value layout and retention
contract rather than adding another value representation.

`Arena` owns `ValueColumns`, dense root layouts, and reusable preparation `Layouts`
scratch. Before every preparation it resets scratch content and traversal, retaining
the outer capacity; warmed empty snapshots do not allocate bookkeeping. `Default` starts empty.
`add_output(&mut Bindings, NodeId, OrdinaryType) -> OutputId` prepares the root
layout and associates an exact atomic endpoint, preserving reused slot identity.
Leaves are installed at the first successful publication; no default payload
is invented. Root allocation follows existing endpoint construction, while
fallible payload preparation propagates NodeResult before any publication.
`borrow<T: GlobalValue>(&Bindings, Input<Atomic<T>>) -> NodeResult<ValueSlot<T>>`
checks input validity and selects prepared typed positions;
`values() -> &ValueColumns` exposes read-only storage to typed projection.
`write<T: GlobalValue, W: Wake>(&mut Bindings, Output<Atomic<T>>, T::Value,
EngineTime, &mut W) -> NodeResult` checks endpoint lifetime and publication time,
prepares the complete owned replacement before committing, then publishes through
the existing bindings. No name, schema or dynamic type lookup occurs
in a read or write. Existing GlobalValue retention boundaries remain explicit.

Preparation failures leave the held value and publication state unchanged.
Successful replacement reclaims old descendants; repeated replacement and slot
reuse do not grow storage without bound. Binding generations govern stale
endpoint handles. Empty lists are present values, not invalidations.

Acceptance: atomic list, tuple and nominal storage; independent retained
snapshots; empty/equal publications; failed preparation leaves old state;
repeated replacement reuses storage; graph-scope reuse preserves generation.
Mutants: alias a retained list; skip reclaim; publish before prepare succeeds;
treat an empty list as invalid; accept a mismatched root shape.

An OrdinaryType::Enum root reserves its one i64 physical slot during construction
and starts with no publication. Generated enum markers set PREPARED_SCALAR, so
write skips generic payload preparation/reservation and commits to that typed
slot directly. The compile-time marker selects this path; there is no per-value
schema test, name lookup, allocation or invented published default member.

prepare_output<T:PreparedValue>(&Bindings,OutputId,&T::Bounds)->NodeResult
installs all finite descendants after endpoint construction and before start,
without a publication. destination<T>(&Bindings,Output<Atomic<T>>)->NodeResult<ValueSlot<T>>
validates token generation and requires installed storage without requiring a
previous value tick. values_mut() permits complete-preflight independent copies;
publication remains a separate final Bindings operation in hgl-prepared-store.

OptionalField contributes one descriptor position to the root layout. Complete
snapshot replacement delegates presence retention and reclamation to the
optional field marker; an all-unset struct remains a valid published root.

RecursiveReference is one descriptor position when calculating finite root
widths. Concrete recursive payload operations remain compiler-selected typed
GlobalValue/PreparedValue implementations with exact nominal schemas.
