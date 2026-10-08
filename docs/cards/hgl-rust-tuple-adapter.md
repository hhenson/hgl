# Card: hgl-rust-tuple-adapter

Own cold admission of private Tuple observation storage before emission.
The tuple_adapter module has an initial 60 source-line budget within the unchanged
hgl-rust crate limit. It uses execution_proof and checked IR only.

Public surface: `standalone(Plan)` and `prepared(Plan)` return `Result<Plan, Issue>`.
The former diagnoses owning snapshot uses because standalone construction has no
finite observation-slot preparation contract. The latter calls the existing
whole-adapter execution proof once; unproved owning snapshot adapters reject.
Approved plans move through unchanged. Source canonical types and ordinary native
Tuple construction remain unchanged. No runtime lookup, allocation or type test
is introduced; no private optional slot is admitted without preparation.

Acceptance: finite shared/own Tuple evals execute with prepared descendants;
standalone and unproved growing-generator observation programs fail before
emission with an ordinary uncatalogued backend profile diagnostic.

Own-output assignments are included in the observation-slot scan. Runtime helper
calls requiring unprepared owning text arguments reject before eval emission,
using value_calls' cold proof; standalone/constant ordinary text ABI is unchanged.
