# Card: Cold native values

The native_value module of hgl-value-model contains independently owning NativeValue and its statically retained Operations callbacks. new accepts already validated owned provider bytes; data and operations expose them only to checked compiler execution. Clone invokes provider copy into an independent owner. Operations provides copy/equal/hash/compare; the enclosing Ty carries canonical identity and declared capabilities.

This is cold execution data, not a provider discovery mechanism. Checking never constructs or executes it. The layer depends only on hgl-types and does not depend on Store, source parsing, semantic admission or generated program assembly. Source facade paths preserve existing consumers. Provider failures remain fallible ordinary evaluation results.

Initial module ceiling: native_value 50 lines, within the unchanged value-model aggregate.

A fresh Native helper result passes typed fallible ownership preflight before the
validated capture encoder receives it. Provider error codes survive this boundary;
invalid returned data must not reach the capture invariant assertion. Generated
controls inject a provider copy failure, verify its exact coded result, and count
one semantic helper call for each failed, empty, and nonempty invocation.
