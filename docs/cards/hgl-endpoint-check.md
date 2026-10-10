# Card: hgl-endpoint-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `endpoint_check` module of `hgl-semantics` (`crates/hgl-semantics/src/endpoint_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Independent checking for runtime endpoint operations and scalar payload
boundaries. Dependencies hgl-source/hgl-rust-ir; budget 220 source lines.
Public: set_call, scalar, endpoint_metadata, injected_clock, clock_property,
require_payload, endpoint_call. This extraction preserves existing scalar/set
operations, metadata arity/roles, readonly clock properties and payload proofs.
Structural endpoints require explicit derived delta observations at ordinary
payload boundaries. It performs no graph construction or runtime effects.
Acceptance: existing set, clock, signal, metadata and delta proof regressions.
`capability_payload` resolves an injected marker for capability operation checks.

`capability_function(receiver,method,args,env,stopping)` checks source alarm
operations against injected identity, exact argument types and hook phase.
Only a boolean stopping flag crosses this independent checker boundary; graph
phase management remains in the compiler. Depends also on hgl-value-bind.

Input activation/passivation and own-output metadata are admitted only for the
admitted scalar endpoint types, including enums. Non-scalar publication shapes
are rejected before emission; new structural publication shapes do not implicitly extend these operations.
Structural input valid/modified/last_modified observations remain admitted.
Acceptance includes all fixed-list/map/tuple/nominal shapes for these boundaries,
plus the existing scalar operation and structural input observation paths.

inject_capability binds declared capabilities to the lexical environment. arguments evaluates capability operands in source order through a typed Issue callback and checks payload access.

Set insert/remove are strict output operations, distinct from tolerant
upsert/discard. They retain scalar key checks and evaluation ownership.
