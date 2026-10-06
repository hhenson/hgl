# Card: hgl-rust-key-origins

Cold discovery of complete sparse key constants. Depends on hgl-rust-ir and
hgl-static-values; budget 180 source lines.

`node_keys(&Node) -> Vec<Value>` walks configuration, lifecycle hooks, handlers
and generators in lexical order. Immutable local/helper origins support proof
of known complete identities without replacing emitted local accesses. Mutable
origins remain excluded. Provider recipes are never executed by discovery.
Nested lexical branches receive independent origin scopes. Returned values seed
cold domains through existing exact generated preparation operations.
