# Card: hgl-node-error

Status: accepted

Graph failure vocabulary formerly defined in hgl-types, which reexports it.
NodeId, Phase, NodeResult and NodeError retain graph ownership and context.
NodeError::coded attaches an originating stable code; new leaves it absent.
NodeError.cleanup preserves teardown failures alongside the primary failure.
No runtime message classification. Budget 100 source lines, no dependencies.

finish(primary, cleanup) preserves the primary error and attaches a cleanup error when both fail; hgl-types reexports it.
