# Card: hgl-native-contracts

Data-only package contracts use versioned `.hgl-native` sidecars alongside HGL
interfaces. Library loading and file discovery feed the same contract records;
checking parses and validates metadata without loading code or running helpers.
Uses library and struct_names; module budget 350 lines, within the unchanged
hgl-semantics crate budget. No new crate or external dependency.

Public `Contracts`, `Descriptor`, reexported `Scalar`, and `Function` retain module/provider
identity, fingerprint, exact canonical scalar capabilities, readonly owning
helper signatures, target symbol, permitted phases and observable effects.
`sidecar`, `parse`, `validate`, `scalar`, and `function` admit only this exact
ordinary scalar slice. `Descriptor::verify_image` rejects a mismatched selected
provider identity/fingerprint before execution; activation is a later layer.

Descriptor version 1 targets `hgl-rust-1`. Scalar records map a declaration to an
existing canonical identity and require owning copy/text. Aliases share all
capabilities. Value records declare exact scalar parameters/results, an owned
result, read-only parameters, a Rust symbol, explicit setup/cold/evaluation/start/
stop phases and effects. Required constant evaluation is never a permitted phase.
Signal, borrowing, mutation, generic signatures, default arguments and blocking
evaluation remain rejected in this slice. Target bindings never infer operations
from native implementation types. Missing scalar/helper records fail loading.

Acceptance: authored Std fixture identities/capabilities, canonical aliases,
descriptor-only checking, exact helper signatures, missing/incompatible contracts,
mode/phase/signal rejection and image fingerprint comparison. This foundation
does not yet execute native scalar values or activate provider images.

Scalar/helper record parsing is owned by hgl-source::native_descriptor. Its plain
Function data retains exact parameter/result annotations, replacing synthetic
callable tokens. Canonical comparison and source declaration validation share the
same semantic path. No checker-time execution is introduced by the separation.
