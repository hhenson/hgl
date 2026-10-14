# Card: hgl-value-operators

The value_operators policy selects exact ordinary binary result types. It uses hgl-source and has no internal module dependencies; initial budget 120 lines.

Public surface: binary_type, reexported by value_check. Any comparisons defer actual contained capability to execution or required evaluation. Boolean equality/order accepts BOOL-1's false-before-true relation without numeric conversion. Existing numeric, temporal and composite typing remains unchanged.

Acceptance: complete direct/boxed Boolean comparison matrix, Any mixed identity shortcuts, required constant capability diagnostics and existing value operator tests.

Result equality admission requires equality of both complete ordinary payload
types, independently of the selected tag. Contextual capture comparison uses the
same requirement. Harness/interpreter equality checks it before comparing tags,
so inactive unsupported native payloads cannot bypass the contract.
