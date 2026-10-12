# Card: hgl-native-contracts

Data-only package contracts use versioned `.hgl-native` sidecars alongside HGL
interfaces. Library loading and file discovery feed the same contract records;
checking parses and validates metadata without loading code or running helpers.
Uses library, struct_names and checked ir; module budget 350 lines, within the unchanged
hgl-semantics crate budget. No new crate or external dependency.

Public `Contracts`, `Descriptor`, reexported `Scalar`, and `Function` retain module/provider
identity, fingerprint, exact canonical scalar capabilities, readonly owning
helper signatures, target symbol, permitted phases and observable effects.
`sidecar`, `parse`, `validate`, `scalar`, `function`, `bind`, and `intern` admit only this exact
ordinary scalar slice. `Descriptor::verify_image` rejects a mismatched selected
provider identity/fingerprint before execution. Header records and optional image
locations are inert hgl-source::native_package data.

Descriptor version 1 targets `hgl-rust-1`. Scalar records map a declaration to an
existing canonical identity and require owning copy/text. Aliases share all
capabilities. Value records declare exact scalar parameters/results, an owned
result, read-only parameters, a Rust symbol, explicit setup/cold/evaluation/start/
stop phases and effects. Required constant evaluation is never a permitted phase.
Signal, borrowing, mutation, generic signatures, default arguments and blocking
evaluation remain rejected in this slice. Target bindings never infer operations
from native implementation types. Missing scalar/helper records fail loading.

Logical aliases remain one canonical HGL scalar. The `hgl-rust-1` target requires
compatible selected mappings: each canonical identity uses the same provider,
image path, semantic fingerprint and explicit Rust marker. Different mappings
reject during descriptor checking, before image loading or Rust emission; this
ABI provides no conversion between opaque representations. A provider identity
selects one image path and fingerprint across modules. Compatible modules may
contribute distinct scalar records, all of which are retained in generated code.

Acceptance: authored Std fixture identities/capabilities, canonical aliases,
descriptor-only checking, exact helper signatures, missing/incompatible contracts,
mode/phase/signal rejection and image fingerprint comparison. Native nominal scalar
source values bind through these validated contracts and execute through explicit
provider contexts and generated typed callbacks.

A selected ordinary scalar helper with an image record binds its explicit symbol
and exact setup/cold/node lifecycle phase, without reading or executing image code while checking.
Generated code validates every selected descriptor against the statically linked
image before configuration or graph construction. Missing image contracts and
forbidden phases reject binding; missing target files or incompatible callback
prototypes reject the generated target build. No Provider injection is used for
these checked image calls. Executed setup and cold configuration use statically
generated typed callbacks; required constant contexts remain unavailable.

Library sets Decl::ordinary_native only after contract validation. This preserves
legacy native input-view declarations while treating checked ordinary native const
functions as readonly value helpers. Required constant execution stays unavailable.

Scalar/helper record parsing is owned by hgl-source::native_descriptor. Its plain
Function data retains exact parameter/result annotations, replacing synthetic
callable tokens. Canonical comparison and source declaration validation share the
same semantic path. No checker-time execution is introduced by the separation.

The authored fixture sidecar resides in native/stdlib beside the default target
interfaces. Ordinary library discovery selects it once; fixture-specific API tests
use that same root, so shared Std declarations require no bespoke test injection.

Executable package binding projects each scalar's explicit target marker along
with its portable contract. A missing marker rejects that executable binding;
the checker never guesses a Rust type name from an HGL alias or canonical string.
The generated image gate checks the marker's full native ABI contract before
setup, cold preparation or runtime helper execution.

Portable Image, Scalar and Function metadata belongs to hgl-value-model, below
source parsing and checked execution. Its existing 40-line module ceiling moves
unchanged from hgl-types. hgl-source exposes the portable records and preserves
its native_descriptor facade. Runtime Store contracts remain independent typed
ABI operations and gain no dependency on this compiler metadata. Generated
checked IR uses the source facade; no source/provider spelling changes.

Binding records checked setup/cold phase on the returned Value invocation, not the interned provider method. Pure ABI result-bound functions are separate descriptor metadata and never execute HGL helper bodies during checking or provisioning. Missing bounds reject owning evaluation helpers before code generation.
