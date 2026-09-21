# Card: hgl-nested

Status: implemented for the admitted dynamic slice; see [coverage](../runtime-implementation.md).

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
