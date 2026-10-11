# 0005 — Retained Any descriptors cross generated module boundaries

Status: accepted implementation decision; no language extension.

## Decision

An owning Any encoding retains typed references to static compiled descriptors
alongside its bytes. Nested headers identify entries in that owner's metadata,
not positions in the producer's or consumer's generated layout table. Bounded
views preserve descriptor context. Retaining a subframe copies payload bytes and
relocates descriptor headers into independently owned metadata. Descriptor
references have static lifetime; source replacement and teardown cannot affect
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

Under decision 0004, transfer 130 lines of the existing Any operation allowance
from Types to Store with the moved recursive operation executor:

- Types any_values: 450 to 320.
- Store any_values: 350 to 480.

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
