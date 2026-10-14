# Card: hgl-rust-keyed-graphs

The public `keyed_graphs` module in `hgl-rust-codegen` emits the checked Spec
outputless dynamic graph-loop profile. It uses checked IR and HGL source types.
`member(plan,id)` recognizes the synthetic boundary source.
`child(plan,keyed,members,graph,inputs)` emits a keyed child description with no
output and exact source-input boundaries. The member uses `Step::Key`; whole
temporal captures retain their original producer identities and REF boundaries.
`node(plan,id,shape)` emits a typed owner using prepared keyed child pools.
`support()` emits cold boundary adaptation and activation binding code.
`preparation(plan,recursive_prepare)` emits cold owner preparation clauses for
the enclosing recursive child-preparation function.

The finite prepared target reserves collection domains, child instances, wake
maps and change buffers before startup. Membership updates preserve child state;
removal stops and retires the current scope; recreation consumes another fresh
instance. REF sample/follow/designation adaptation is selected cold, before node
hooks. Successful finite ticks allocate no child graphs or change buffers and do
not dispatch by source names or payload types. Unknown domains and unproved
activation counts require a separate target profile; this module does not claim
an allocation bound for arbitrary dynamic inputs. Captured node failures use
the explicit compiled capture owner inside each member. Uncaught failures escape
the keyed owner through the ordinary graph failure path.
