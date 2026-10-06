# Card: hgl-deadlines

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `deadlines` module of `hgl-kernel` (`crates/hgl-kernel/src/deadlines.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Share one indexed deadline heap between the kernel and child manager. May use
`hgl-types`. Budget: 100 source lines.

Surface: `Deadlines::{default,reserve,first,set,remove,clear}`. Slots are usize;
deadlines are EngineTime. FOREVER cancels. Each reserved slot has at most one
entry; replacement and cancellation cost O(log n), earliest lookup O(1).
Reserve may allocate during structural changes. The other methods never do.

Acceptance: moving deadlines both ways, cancellation at every heap position,
repeated replacements, equal-time ordering and empty queues. Compare randomized
operations with a simple test-only ordered model. Mutants: omit the moved
slot's position update or repair only in one direction; both must fail.

```rust
fn clear(&mut self);
fn reserve(&mut self, slots: usize);
fn first(&self) -> Option<(EngineTime, usize)>;
fn set(&mut self, slot: usize, time: EngineTime);
fn remove(&mut self, slot: usize);
```

`Deadlines` implements `Debug` and `Default`. Reserve never shrinks slots.
