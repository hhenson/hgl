# TSL and TSB runtime slice

Status: implemented through the Rust API; [validation](runtime-implementation.md).
The [contract rulings](https://github.com/hhenson/hgraph_spec_audit/blob/main/runtime/validation/fixed/README.md) remain the oracle.

Recursive `Kind` descriptions replace the scalar-only dictionary and REF
shapes. `hgl-endpoints` owns storage records and scope mailboxes;
`hgl-bindings` owns binding, notification and retirement policy. Existing
crate budgets remain unchanged.

| Area | Implementation contract |
|---|---|
| Shape | Describe scalars, fixed list length, named bundle fields, dictionary child shape and REF target shape independently of bindings. Reject incompatible shapes before changing any binding. |
| Storage | Keep scalar columns and generation-checked handles. Allocate fixed child slots when the graph instance is built; use dense positional access. Field-name lookup belongs to graph construction. TSB values preserve all declared fields; invalid child values are nil. |
| Binding | A fixed input retains its children through whole-output, child-reference and empty bindings. Each child can itself be peered or assembled. REF designation identity includes its nested structure; equal designations cause no additional tick. Rebinding preserves unchanged child targets. |
| Observation | Maintain assembled state from child events; cache aggregate time and modification instead of requiring read-time scans. Retain child-change time while the structure is valid; reset it and descendant observations to `never` when invalid. Sampling affects input views, never producer time. Parent deltas preserve child deltas and fixed positions. |
| Notification | Carry child notifications through every assembled ancestor, even when no child reads modified. Activity controls scheduling, not readability. |
| Lifetime | Retire a removed TSD member's entire fixed subtree at the next cycle. Clear descendant bindings and expire saved references before reusing slots. |
| Child graphs | Reuse the existing scope and timer machinery. Bind nested inputs before start; preserve their peering and sample state. Detach recursively on stop and removal. |

The fixed-child path is separated from keyed membership. A dictionary's
map and removal bookkeeping are appropriate for dynamic keys; fixed fields
need neither insertion nor lookup on each tick. Fixed input slots survive unbinding so cached child handles stay valid.

Implement against the accepted traces, then exercise shape rejection,
passivity, duplicate notifications, stale descendant handles and bounded
churn. Measure steady nested ticks and REF switches after warm-up with no
allocation; compare equivalent C++ graphs. Preserve the existing crate
budgets and scalar path. New module boundaries and their API cards must be
settled before extending that path.

This slice concerns the runtime API. Compiler lowering and growing TSL need
separate coverage; neither is established by these comparison results.
