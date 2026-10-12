# 0005 — Retained Any descriptors cross generated module boundaries

Status: accepted implementation decision; no language extension.

## Decision

An owning Any encoding retains typed references to static compiled descriptors
alongside its bytes. Nested headers identify entries in that owner's metadata,
not positions in the producer's or consumer's generated layout table. Bounded
views preserve descriptor context. The compiled owning-copy codec reconstructs
nested headers in independent metadata while preserving provider copy operations.
Descriptor references have static lifetime; source replacement and teardown cannot affect
retained payloads or their dispatch.

Types owns the descriptor/data vocabulary and independent storage. Store owns
execution of recursive encoded equality, partial order and hashing, including
capabilities and floating-point restrictions. Compiled descriptors supply those
execution callbacks, so Types has no dependency on Store. Successful evaluation
uses direct typed descriptor references with no name registry, pointer decoding,
or per-value type inspection.

Each descriptor also supplies a fallible cold capture callback. Its type-erased
carrier is generated checked IR, owned independently and downcast only at the
compiler's cold capture boundary. Missing or incompatible carriers are errors.
This preserves the Types-to-Model dependency wall without creating a duplicate
ordinary-value model in Types. Provider execution and error cleanup remain on
the existing execution path.

Finite byte bounds reserve independent payload and metadata capacity: every
populated nested descriptor requires an eight-byte header, bounding metadata
slots by the encoded byte width. Prepared copy and composition check both
capacities before replacing an owner and allocate neither during successful
evaluation. Internal buffer/callback APIs change together; the encoding is not
a public serialization format.

## Budget ownership

Under decision 0004, transfer 30 lines of the existing Any operation allowance
from Types to Store with the moved recursive operation executor:

- Types any_values: 450 to 420.
- Store any_values: 350 to 380.

The combined allowance remains 800 lines. Types' 1200-line crate cap, Store's
6860-line crate cap, and all other caps remain unchanged. No new crate or
dependency is introduced.

## Evidence

Two independently emitted modules with different layout tables reproduce the
old descriptor substitution. Controls cover nested retained fields, foreign
cold capture, native-returned boxes, differing values and capability failures,
independent prepared descriptor storage, and incompatible cold carriers. Existing
generated Any allocation and provider-failure tests remain required in debug and
optimized builds.

## Canonical identity across emitted modules

Module-local Rust nominal marker types cannot identify the same imported HGL
declaration across independently emitted modules. Descriptor identity therefore
uses TypeId of shared Types IdentityChunk markers encoding exact canonical UTF-8
name bytes and explicit chunk lengths. A balanced tuple of chunks bounds Rust
type nesting logarithmically for long admitted names. This is an injective
encoding, including generic arguments and trailing zero bytes; it has no hashes,
registry, lazy initialization or runtime normalization. Existing canonical source
normalization occurs before emission, so normalized atomic scalar spellings
retain their common identity. Distinct nominal origins remain distinct even with
identical payload layouts.

## Native integration

Native copying remains a two-pass operation. The sizing sink invokes provider
preflight and records its first failure without executing copy callbacks; only
a successful complete preflight permits mutation. Cold descriptor readers return
NodeResult and propagate the original provider code without panicking. Capture
moves its newly retained native owner into checked data, avoiding a second copy.

Types owns scalar/header decoding, optional-field views, and bounded frame
traversal with caller-supplied fallible callbacks. Store selects recursive
comparison, capability and Native dispatch semantics. The original raw byte
relocation helper is removed: recursive compiled copy already reconstructs the
sidecar and is required for Native owning retention. Both crates retain their
original crate caps; the module allowance remains 800 lines combined.
