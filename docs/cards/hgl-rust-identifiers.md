# Card: hgl-rust-identifiers

Private compiler identifier encoding shared by nominal struct/enum markers and Any descriptors. Initial budget 40 lines; no other module dependencies.

Surface: crate-private identifier(source). Complete UTF-8 bytes are encoded as fixed-width decimal octets, without hashing or source-name collisions. Generated private names do not change canonical source type identity or capture metadata. No encoding occurs during evaluation.

Acceptance: exact canonical nominal specializations and arbitrary compiled box descriptors coexist in generated Rust, with import alias identity retained.

any_identity(source bytes) emits the shared Types IdentityChunk const-generic
marker for each exact UTF-8 chunk, retaining its byte length. Balanced tuple
composition bounds generated type nesting logarithmically in name length.
TypeId observes this shared compiler-generated identity in constant time across
independent modules; no hashing, registry, lazy initialization or name lookup is
needed. Canonical source normalization occurs before identity emission.
