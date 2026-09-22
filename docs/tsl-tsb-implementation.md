# TSL and TSB runtime slice

Status: design review; implementation waits for the unresolved
[contract decisions](runtime_spec/validation/fixed/README.md).

The current scalar-shaped endpoint model cannot represent this slice.
`Dictionary(ScalarType)` and the REF `dictionary` flag must become recursive
shape descriptions. Adding more scalar-specific collection variants would
leave nested fields, dictionary children and REF targets incompatible.

| Area | Required change |
|---|---|
| Shape | Describe scalars, fixed list length, named bundle fields, dictionary child shape and REF target shape independently of bindings. Reject incompatible shapes before changing any binding. |
| Storage | Keep scalar columns and generation-checked handles. Allocate fixed child slots when the graph instance is built; use dense positional access. Field-name lookup belongs to graph construction. |
| Binding | A fixed input retains its children through whole-output, child-reference and empty bindings. Each child can itself be peered or assembled. REF designation identity includes its nested structure. |
| Observation | Derive assembled validity and modification from children. Sampling affects input views, never producer time. Parent deltas preserve child deltas and fixed positions. |
| Notification | Carry child notifications through every assembled ancestor, even when no child reads modified. Activity controls scheduling, not readability. |
| Lifetime | Retire a removed TSD member's entire fixed subtree at the next cycle. Clear descendant bindings and expire saved references before reusing slots. |
| Child graphs | Reuse the existing scope and timer machinery. Bind nested inputs before start; preserve their peering and sample state. Detach recursively on stop and removal. |

The fixed-child path should be separated from keyed membership. A dictionary's
map and removal bookkeeping are appropriate for dynamic keys; fixed fields
need neither insertion nor lookup on each tick. The current projection code
also releases and recreates children on unbind; that must not invalidate a
node's cached fixed-child handles.

Implement against the accepted traces, then exercise shape rejection,
passivity, duplicate notifications, stale descendant handles and bounded
churn. Measure steady nested ticks and REF switches after warm-up with no
allocation; compare equivalent C++ graphs. Preserve the existing crate
budgets and scalar path. New module boundaries and their API cards must be
settled before extending that path.

This slice concerns the runtime API. Compiler lowering and growing TSL need
separate coverage; neither is established by these comparison results.
