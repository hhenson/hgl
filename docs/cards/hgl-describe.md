# Card: hgl-describe

Status: recursive-description infrastructure; compiler implementation is later.

Build and instantiate the plain descriptions from [hgl-plan](hgl-plan.md).
Uses `hgl-types`, `hgl-store`, `hgl-kernel`, `hgl-plan`. Budget remains 500 lines.

The existing `Buildable`, `Registry`, `Builder`, `NodeRef`, `Ports` and
`instantiate` entry points remain. Description types and `BuildError` are
re-exported from hgl-plan. Registry construction remains explicit; no lookup
or allocation is added to ordinary ticks.

## Surface additions

- `Registry` is cloneable; constructors remain function pointers. A host node
  may retain its registry and child templates while constructing its instances.
- `Ports::shaped_input(name)`, `shaped_output()` allocate the declared recursive
  shape and return endpoint IDs. `store()` exposes shared projection/typed-handle
  reads; `registry()` and `children()` expose construction metadata. Scalar
  `input`, `output` and `scalar` retain their checks and single-take contract.
- `Builder::connect_path(source, source_path, target, input, target_path)`
  extends whole-port `connect`. `children(node, templates)` attaches reusable
  child descriptions. `finish` preserves existing FIFO topological ranking and
  remaps every edge while leaving child-local positions unchanged.
- `instantiate_complete(description, registry, store)` returns
  `BuiltGraph { graph, inputs, outputs }`; its `input(port, store)` and
  `output(port, store)` resolve checked boundary paths. Existing `instantiate`
  returns just the graph.
- `instantiate_child(template, registry, store, owners, key, now)` constructs
  a child in the caller's current scope and binds its boundary before start.
  The caller retains lifecycle ownership through `Ctx::create_child`/`Children`.

Validate all nested descriptions before allocating ports. NodeType's
`child_graphs` count must match the supplied templates. A keyed template joins
an owner's dictionary member; an unkeyed one exposes the whole output. REF
carriers do not change that shape check. All targets, including boundary targets,
obey non-overlap. Fixed missing children stay unbound.

A boundary preserves live REF subscriptions recursively; copying only today's
designation is insufficient. Bind valid targets with current-cycle sampling,
without changing producer times. Ordinary edges constructed before start bind
silently. Distinct child instances keep independent ports and state. Construction
failure inside `Ctx::create_child` uses its existing scope rollback.

Acceptance: the scalar description tests remain; new rejection tests pin
GRF-4/6/7/9. Described child graphs replay all five validated boundary scenarios,
including parent-capture rebinding, removed descendant reads and fresh state.
No extra assembly node, per-tick name lookup or value copy is introduced.

Current construction limit: a boundary path cannot descend through an input
following a live REF. Capture that whole target and project inside the child.
Reject the unsupported boundary before allocation; taking its current target
would silently lose later rebinding.
