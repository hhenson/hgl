# Card: hgl-rust-identifiers

Private compiler identifier encoding shared by nominal struct/enum markers and Any descriptors. Initial budget 40 lines; no other module dependencies.

Surface: crate-private identifier(source). Complete UTF-8 bytes are encoded as fixed-width decimal octets, without hashing or source-name collisions. Generated private names do not change canonical source type identity or capture metadata. No encoding occurs during evaluation.

Acceptance: exact canonical nominal specializations and arbitrary compiled box descriptors coexist in generated Rust, with import alias identity retained.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.
