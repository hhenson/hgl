# Card: hgl-types-any-values

The any_values vocabulary in hgl-types owns compiled box descriptors and independently retained encoded ordinary data. No runtime or third-party dependency is introduced. Module budget: 320 lines; 130 lines of the original allocation are transferred to Store with recursive execution.

Public surface: AnyPlan {identity, operation, capabilities, execution, capture}, AnyField {plan, optional}, AnyOperation, AnyValue, AnyRef, AnyView, AnyExecution, ColdCapture, Frames, AnyBuffer and AnyWidth. AnyValue exposes from_encoded, plan, data, reserve, capacity, copy_prepared, encode_prepared, retain, capture, equal, compare and key_hash. AnyPlan exposes equal, compare and hash. AnyBuffer writes and frames typed bytes, appends descriptor-bearing views and writes descriptor headers; AnyWidth measures the same encoding before publication. Frames iterates compiler-produced length-framed children.

Descriptors hold compiled operations and exact type identity, with no name lookup or payload type validation during successful evaluation. They cover arbitrary admitted ordinary layouts, recursively including family members and boxes. Encoded bytes and descriptors are an internal compiler/runtime contract, not a user serialization API. Whole family boxing selects a concrete member; nested family fields retain their tags.

Cold retain/reserve may allocate. Prepared copy/encoding checks destination capacity first and uses independently reserved storage. Actual comparisons and hashes preserve recursive capabilities, optional presence, unordered collection equality, partial order and non-NaN key rules. Missing capabilities return value.capability.

Acceptance: hgl-store Any allocation/retention tests and generated compiler allocation probes, including boxed structural delta independence and empty container capability failures.

Each owner retains descriptor references in a separate typed sidecar. Encoded headers index that owner's metadata, never a generated module table. Views preserve their owner context through frame slicing; retained subframes relocate headers into independent metadata without serializing pointers. Every populated nested header consumes eight bytes, so finite byte bounds also reserve an upper bound on descriptor slots. Successful prepared copies and compositions allocate neither bytes nor metadata.

AnyPlan execution callbacks select Store's compiled recursive operations. Its optional capture callback returns an independently owned, type-erased checked carrier only at cold compiler boundaries. AnyValue::capture validates that carrier through a fallible cold downcast; missing or incompatible callbacks are construction errors. No compiler/Model dependency, lookup, or downcast enters evaluation. Static descriptor references outlive every retained owner.

The internal ABI changes: data returns a metadata-preserving AnyView rather than bare bytes; encode_prepared receives an owning AnyValue buffer. AnyRef holds an optional plan and borrowed view; AnyView offers slice, boxed and retained. AnyPlan has no module-local index. Raw encodings are compiler/runtime-private and are not portable serialization.

IdentityChunk<BYTES,LENGTH> is a shared zero-sized compiler identity marker.
Balanced tuples of exact chunks distinguish canonical names, generic arguments,
and byte lengths across separately emitted modules. It carries no runtime data;
its TypeId provides the existing constant-time descriptor identity observation.
