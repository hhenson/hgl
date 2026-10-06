# Card: hgl-node-error

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `node_error` module of `hgl-types` (`crates/hgl-types/src/node_error.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Graph failure vocabulary formerly defined in hgl-types, which reexports it.
NodeId, Phase, NodeResult and NodeError retain graph ownership and context.
NodeError::coded attaches an originating stable code; new leaves it absent.
NodeError.cleanup preserves teardown failures alongside the primary failure.
No runtime message classification. Budget 100 source lines, no dependencies.

finish(primary, cleanup) preserves the primary error and attaches a cleanup error when both fail; hgl-types reexports it.
