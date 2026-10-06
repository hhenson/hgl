# Card: hgl-recursive-types

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `recursive_types` module of `hgl-semantics` (`crates/hgl-semantics/src/recursive_types.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Finite recursive declaration and specialization batches from ADR0012 and finite
recursive atomic publications. Uses source, library, struct-names, shape-obligations, value-access and enums; budget350.
`resolve(library, declaration, arguments, resolve_field)` returns no batch for an
acyclic declaration, otherwise a complete canonical `Ty::Recursive`. The callback
resolves ordinary nonrecursive field types and concrete generic arguments.

Determine cyclic components from declaration-owned field references. Every edge
within a component must be a direct optional atomic target with null default;
container, argument-expanded, non-atomic, required and cross-module cycle edges
fail before concrete formation. Specialization expansion follows only unchanged
source parameters or closed argument expressions. Canonical exact nominal keys
bound the queue; every referenced concrete edge resolves in the emitted batch.
