# Card: hgl-any-values

The `any_values` policy in hgl-semantics checks ANY-1–5 against admitted ordinary types. It may use ir, value_eval, value_access, endpoint_check, family_values and scalar_keys; its initial module budget is 420 lines within the unchanged crate budget.

Public surface: checked, retain, contents, capability, capable, binary, equal_data, validate_key, fields and contains_any. Construction retains exact concrete family identity and independently owns every ordinary descendant. Atomic and delta Any wrappers normalize to the same whole box leaf. There is no closed builtin payload union.

Capability selection is type based, including empty ordinary containers. Recursive actual box key validation requires contained Eq/hash and excludes NaN, even when comparisons could short circuit on differing identity. Executed failures retain value.capability; required constant checking maps that failure separately. Equality and partial ordering retain empty-box and differing-contained-type shortcuts from ANY-3. Timezone and zoned scalar ordering remains unavailable.

Acceptance: shared any_values suite; independent retained recursive aggregates and structural delta payloads; exact nominal identity.
