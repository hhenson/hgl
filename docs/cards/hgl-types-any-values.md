# Card: hgl-types-any-values

The any_values vocabulary in hgl-types owns compiled box descriptors and independently retained encoded ordinary data. No runtime or third-party dependency is introduced. Module budget: 420 lines; 30 lines of the original allocation are transferred to Store with recursive execution.

Public surface: AnyPlan {identity, operation, capabilities, execution, capture, validate, copy}, AnyField {plan, optional}, AnyOperation, AnyValue, AnyRef, AnyView, AnyExecution, ColdCapture, ColdValidate, Frames, AnyBuffer and AnyWidth. AnyValue exposes from_encoded, plan, data, reserve, capacity, copy_prepared, encode_prepared, retain, capture, equal, compare and key_hash. AnyPlan exposes equal, compare and hash. AnyBuffer writes and frames typed bytes, executes provider copies and writes descriptor headers; AnyWidth measures the same encoding before publication. Frames iterates compiler-produced length-framed children.

Descriptors hold compiled operations and exact type identity, with no name lookup or payload type validation during successful evaluation. They cover arbitrary admitted ordinary layouts, recursively including family members and boxes. Encoded bytes and descriptors are an internal compiler/runtime contract, not a user serialization API. Whole family boxing selects a concrete member; nested family fields retain their tags.

Cold retain/reserve may allocate. Prepared copy/encoding checks destination capacity first and uses independently reserved storage. Actual comparisons and hashes preserve recursive capabilities, optional presence, unordered collection equality, partial order and non-NaN key rules. Missing capabilities return value.capability.

Acceptance: hgl-store Any allocation/retention tests and generated compiler allocation probes, including boxed structural delta independence and empty container capability failures.

Each owner retains descriptor references in a separate typed sidecar. Encoded headers index that owner's metadata, never a generated module table. Views preserve their owner context through frame slicing; compiled owning-copy codecs reconstruct headers in independent metadata without serializing pointers. Every populated nested header consumes eight bytes, so finite byte bounds also reserve an upper bound on descriptor slots. Successful prepared copies and compositions allocate neither bytes nor metadata.

AnyPlan execution callbacks select Store's compiled recursive operations. Its optional capture callback returns an independently owned, type-erased checked carrier only at cold compiler boundaries. AnyValue::capture validates that carrier through a fallible cold downcast; missing or incompatible callbacks are construction errors. No compiler/Model dependency, lookup, or downcast enters evaluation. Static descriptor references outlive every retained owner.

The internal ABI changes: data returns a metadata-preserving AnyView rather than bare bytes; encode_prepared receives an owning AnyValue buffer. AnyRef holds an optional plan and borrowed view; AnyView offers slice, boxed and field. AnyPlan has no module-local index. Raw encodings are compiler/runtime-private and are not portable serialization.

IdentityChunk<BYTES,LENGTH> is a shared zero-sized compiler identity marker.
Balanced tuples of exact chunks distinguish canonical names, generic arguments,
and byte lengths across separately emitted modules. It carries no runtime data;
its TypeId provides the existing constant-time descriptor identity observation.

NativeCopy and NativeCheck name provider callbacks. AnyPlan.copy selects the
recursive owning codec over AnyView; AnyValue.preflight and check_capacity
validate retention before mutation. AnyBuffer.native uses provider checks in
AnyWidth and copy callbacks in owning sinks. AnyWidth.finish propagates the
first preflight failure. Cold readers preserve provider errors as NodeResult.
AnyView.field, number and index expose bounded compiled data vocabulary.
Frames.equal_by and compare_by iterate frames through caller-supplied fallible
predicates; descriptor selection and operation semantics remain in Store.


`AnyView::from_encoded(bytes, descriptors)` borrows an existing checked compiled
encoding with its independently resolved descriptor context. The caller supplies
metadata only after validating its cold carrier identity; the view owns nothing
and executes no callbacks. `AnyValue::data` uses the same constructor.

Cold descriptor validation uses `ColdValidate = fn(AnyView) -> NodeResult` and
`AnyPlan.validate: Option<ColdValidate>`. Generated descriptors validate complete
typed framing before trusted decoding. Descriptors without that mapping are
rejected at cold construction. `AnyView::checked_boxed() -> NodeResult<AnyRef>`
checks the descriptor index and empty-box payload before returning a borrow.
These checks do not run on the prepared publication path.
