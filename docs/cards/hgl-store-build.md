# Card: hgl-store-build

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `store_build` module of `hgl-store` (`crates/hgl-store/src/store_build.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold recursive allocation of typed value storage and output endpoints. May use
hgl-bindings, hgl-types, hgl-columns and hgl-atomic. Budget: 100 source lines.
Store keeps publication and typed access; this crate owns construction only.

Public surface:

- `scalar<T: Scalar>(&mut Bindings,&mut Columns,NodeId) -> OutputId`: allocate
  or reuse a typed scalar slot with the existing endpoint generation policy.
- `output(&mut Bindings,&mut Columns,&mut Arena,NodeId,Kind) -> OutputId`:
  recursively allocate scalar, atomic and fixed descendants; collection roots
  start without live members.

Runtime-selected shape dispatch is construction-only. Static generated
factories remain available through Store. Existing store scalar, fixed and
dynamic tests verify slot reuse and recursive allocation after this extraction.
