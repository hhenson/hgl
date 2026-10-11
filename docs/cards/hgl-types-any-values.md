# Card: hgl-types-any-values

The any_values vocabulary in hgl-types owns compiled box descriptors and independently retained encoded ordinary data. No runtime or third-party dependency is introduced. Initial module budget: 450 lines.

Public surface: AnyPlan {index, identity, operation, capabilities}, AnyField {plan, optional}, AnyOperation, AnyValue, Frames, AnyBuffer and AnyWidth. AnyValue exposes from_encoded, plan, data, reserve, capacity, copy_prepared, encode_prepared, retain, equal, compare and key_hash. AnyPlan exposes equal, compare and hash. AnyBuffer writes and frames typed bytes; AnyWidth measures the same encoding before publication. Frames iterates compiler-produced length-framed children.

Descriptors hold compiled operations and exact type identity, with no name lookup or payload type validation during successful evaluation. They cover arbitrary admitted ordinary layouts, recursively including family members and boxes. Encoded bytes and descriptors are an internal compiler/runtime contract, not a user serialization API. Whole family boxing selects a concrete member; nested family fields retain their tags.

Cold retain/reserve may allocate. Prepared copy/encoding checks destination capacity first and uses independently reserved storage. Actual comparisons and hashes preserve recursive capabilities, optional presence, unordered collection equality, partial order and non-NaN key rules. Missing capabilities return value.capability.

Acceptance: hgl-store Any allocation/retention tests and generated compiler allocation probes, including boxed structural delta independence and empty container capability failures.

AnyPlan.copy is the statically selected recursive owning-copy codec. AnyBuffer.native appends one provider leaf; Vec executes its provider copy while AnyWidth only counts its width. The caller preflights complete capacity and clears an owner exactly once before append; provider copies never clear sibling data. Clone, retain and prepared copy all use the selected codec, including Native leaves inside boxes and aggregates. Equality/hash/order delegate provider capabilities and propagate their error identifiers.
