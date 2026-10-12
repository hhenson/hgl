# Card: hgl-call-locals

Callsite-local storage identity normalization for emitted ordinary helpers.
Public `prepare(&mut Plan)` assigns each helper declaration a distinct node-wide
ID and rewrites its lexical references, including arguments of nested calls.
Formal argument IDs remain unchanged; nested helper bodies have independent
maps. No expression is evaluated, duplicated or reordered. May use ir and
ir_visit; module budget 100 within the unchanged semantics crate budget.

Acceptance: prepared Any helpers with repeated callsites, nested helper calls,
shadowed locals and independent caller owners retain their values without
allocation in debug and optimized executions.

Internal maximum shares declaration-ID scanning with snapshot destination planning.
