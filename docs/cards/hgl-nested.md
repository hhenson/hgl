# Card: hgl-nested

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `nested` module of `hgl-kernel` (`crates/hgl-kernel/src/nested.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Own a keyed set of child Graph instances. May use `hgl-kernel`, `hgl-store`
`hgl-types` and `hgl-deadlines`. Budget: 350 source lines. No third-party dependencies.

## Surface

`Children::new`, `insert(key, ctx, factory)`, `remove(key, ctx)`,
`evaluate(ctx)` and `stop(ctx)`. The factory receives the shared Store and
returns a Graph plus its chosen boundary handle. Insert returns that handle.
A child starts immediately; removal stops it once, cancels its deadline and
retires its ports. A fresh insertion builds fresh node state.

Evaluate visits only notified or due children, once each at the parent time.
It propagates the earliest remaining deadline through the owning node. The
manager's indexed deadline heap replaces entries in place; stale timers cannot
accumulate under churn. Storage allocation is confined to structural changes.
An outer graph can own the same manager through another level of nesting.

Rules: GRF-15–GRF-18, GRF-21–GRF-23, NOD-11–NOD-13 and TS-23.
Acceptance: the six accepted nested tick traces, lifecycle failure cases and
combined TSD/REF/state/timer case, repeated through an outer owning graph.
Exercise failed child construction/start, sibling isolation, input at a due
deadline, cancellation and same-key recreation. Mutants: omit owner wake,
retain removed deadlines, reuse stopped node state, evaluate a child twice,
or stop a child whose start failed. Every mutant must fail an isolated test.

```rust
fn new() -> Self;
fn insert<T>( &mut self, key: i64, ctx: &mut Ctx<'_>, build: impl FnOnce(&mut Store) -> Result<(Graph, T), Box<NodeError>>) -> Result<T, Box<NodeError>>;
fn remove(&mut self, key: i64, ctx: &mut Ctx<'_>) -> NodeResult;
fn evaluate(&mut self, ctx: &mut Ctx<'_>) -> NodeResult;
fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult;
```

`Children` implements `Debug` and `Default`. Keys are unique per manager.

## Prepared finite instances

`PreparedChild<T>::new(store, owner, factory)` constructs one fresh child in
its own scope before startup and retains its independent boundary T. It does
not run a hook. `boundary()` reads that boundary. `start(ctx)` or
`start_with(ctx, bind)` starts it once under the exact parent scope and node
that prepared it. The latter binds captures in the still-constructing child
scope before hooks start; quiet sampling queues the child's initial work.
`evaluate(ctx)`, `next_scheduled_time()` and `stop(ctx)` drive that instance.
Evaluation before start and foreign-owner activation/stop are errors. Failed
binding or start releases its scope; successfully started nodes stop once,
and the failing start node never receives stop. Repeated stop is harmless.
Never-started prepared instances can be discarded without running stop hooks.
Discarding an unused outer instance also retires its unstarted descendants.
`scope()` exposes the stable mailbox identity for selection-aware notifications.

Finite compiler preparation must construct sufficient independent instances,
reserve target subscriptions and call Store::prepare_collection_inputs before
startup. It must consume fresh instances on reentry and drive only selected
children; no stopped instance is resumed. This surface does not change the
general dynamic Children allocation policy or imply compiler conditional
lowering is complete. The prepared_child regression measures every successful
cycle, including first activation, branch changes, next-cycle expiry and stop,
with a real allocator. Scalar state starts fresh and held old designations do
not revive when a different prepared instance starts.

`PreparedSwitch<T>` owns two cold instance pools, tracks the next unused slot and
one selected child, and matches scope notifications without a lookup table.
`evaluate(enabled,ctx,bind)` stops before switching, binds once using the retained
boundary, and drives only fresh/notified/due work. `stop` stops the active child
and discards unused pools. Exhaustion fails explicitly and never reuses a stopped
instance. `start_bound` provides a borrowed boundary to activation without cloning.
