# Card: hgl-rust-layouts

Emit checked Rust type spellings and nominal ordinary storage layouts. Uses
`hgl-source` and `hgl-rust-ir`; budget 400 source lines. No runtime execution or
third-party dependencies. Expression and statement emission stays in
`hgl-rust-values`; resume control flow stays in `hgl-rust-generators`.

Public surface: `rust_type`, `scalar_type`, `owned_type`, `global_type`,
`global_schema`, `global_markers`. These consume checked concrete types and
plans, never source syntax. The marker collector traverses lifecycle hooks,
ordinary helper expressions and generator bodies, retaining canonical nominal
specializations. Marker implementations stage recursive ordinary storage and
retain the exact existing runtime prepare/install/release contracts. Generated
names encode complete specialization identities without hashing collisions.

Acceptance: unchanged emitted ordinary scalar/list/struct programs and global
metadata; generator locals containing nested ordinary aggregates reference the
same independently owned layouts as existing hook locals and configurations.
