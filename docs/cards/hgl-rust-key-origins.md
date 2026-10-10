# Card: hgl-rust-key-origins

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `key_origins` module of `hgl-rust` (`crates/hgl-rust/src/key_origins.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold discovery of complete sparse key constants. Depends on hgl-rust-ir and
hgl-static-values; budget 180 source lines.

`node_keys(&Node) -> Vec<Value>` walks configuration, lifecycle hooks, handlers
and generators in lexical order. Immutable local/helper origins support proof
of known complete identities without replacing emitted local accesses. Mutable
origins remain excluded. Provider recipes are never executed by discovery.
Nested lexical branches receive independent origin scopes. Returned values seed
cold domains through existing exact generated preparation operations.

Known scalar Map operation keys use the same immutable StaticValues materializer
as sparse constructor keys. Discovery records identity only; operand evaluation
and operation preconditions remain in generated execution.
