# Card: hgl-binding-build

Status: accepted.

Cold endpoint allocation and projection construction. May use hgl-endpoints and
hgl-types. Budget: 160 source lines. Binding event policy remains in hgl-bindings.

Public surface:

- `input(&mut Endpoints,&mut Scopes,NodeId,Kind,bool) -> InputId`: allocate an
  input and its fixed children, recording ownership in the current scope.
- `collection(&mut Endpoints,OutputId,Vec<(i64,OutputId)>)`: establish absent
  finite members. Assert child shape, scope, owner and unattached parent.
- `projections(&mut Endpoints,&mut Scopes)`: after wiring, recursively prepare
  every bound finite collection's input pool and subscriptions. Future members
  remain absent and invalid. Fixed descendants keep dense position identity.
- `member_input(&mut Endpoints,&mut Scopes,InputId,i64) -> InputId`: activate
  an existing finite projection, restore a removal-cycle projection, or allocate
  a projection on the legacy dynamic path.

Construction may allocate; activation of a known prepared member may not.
Projection construction preserves the destination input's active/passive state
and scope. Tests in hgl-store cover nested domains, partial fields, notifications,
retention, removal and first/repeated allocation-free activation.

Projection preparation mirrors an output's bounded runtime-key pool in each input
view; child subscriptions remain attached to matching prepared slot indices.
