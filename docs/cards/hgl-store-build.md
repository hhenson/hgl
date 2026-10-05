# Card: hgl-store-build

Status: accepted.

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
