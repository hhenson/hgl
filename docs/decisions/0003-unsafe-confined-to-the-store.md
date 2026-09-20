# 0003 — `unsafe` is allowed in the store crate, and nowhere else

Status: accepted
Date: 2026-09-19
Exploration: ../explorations/0009-designing-for-speed.md

## Decision

One crate — the store, which holds the value columns, the endpoint table,
handles, binding and notification — may use `unsafe`, from the start. Every
other crate that is part of the runtime inherits `unsafe_code = "forbid"`
from the workspace.

There is one exception outside the runtime: `hgl-alloc-count`, the test-only
global allocator that lets a test assert a tick allocates nothing. A global
allocator cannot be written without `unsafe`; this one forwards every call
unchanged to the system allocator, and is never linked into the runtime.

Inside the store:

- every `unsafe` block carries a `// SAFETY:` comment naming the rule of the
  runtime specification it relies on;
- debug builds assert those rules at every access — owner, rank, thread,
  liveness; release builds trust them;
- the crate's tests run under Miri, and tests run in both debug and release.

The rules relied on are: only a time-series' own node writes it, in its own
evaluation (TS-21); a reader is of higher rank than what it reads (GRF-15,
GRF-24, TS-20); evaluation is single-threaded (NOD-23); a stopped nested
graph is not released in the cycle it stopped (GRF-23).

## Reason

[Decision 0002](0002-performance-parity-with-cpp.md) requires parity with a
C++ runtime built on pointer-stable storage and direct access. The safe
alternatives in exploration 0005 — a tagged value per access, a generation
check per read, moving a node out of its slot to evaluate it — each pay on
every tick. Storage that never moves, and a mutable borrow of a node's own
output held beside shared borrows of its inputs, cannot be expressed in safe
Rust; confining them to one small crate keeps the rest of the code base
checkable by the compiler.

## Given up

- [Decision 0001](0001-rust-end-to-end.md)'s hope of no `unsafe` at all.
- The compiler's guarantee for the store itself: its soundness rests on the
  specification's rules holding, which is why they are asserted in debug
  builds and why a change to one of those rules is now a change to the
  store's safety argument.
- Identical behaviour between debug and release builds.

## Upstream

None.
