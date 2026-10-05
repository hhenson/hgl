# Card: hgl-endpoints

Status: implemented for the fixed collection slice.

Own endpoint records and reusable slot allocation. Re-export recursive shapes
from hgl-types. No
notifications, scalar values or binding policy. May use `hgl-types`.
Budget: 650 source lines. Existing crate budgets remain unchanged.

`Kind` describes scalar, dictionary child, REF target, fixed list element and
length, or ordered named bundle fields. Shapes are immutable after allocation;
comparison and dense child access allocate nothing. Names are resolved while
wiring. `Kind::scalar` is only defined for scalar endpoints.

`InputId`, `OutputId`, `ScopeId`, `Reference`, `Input`, `Output`, `Members`
and `Endpoints` are storage vocabulary. Endpoint records include fixed child
vectors, validity counts and cached observation time. Fields are public for
the binding crate; a Store exposes its actual records only by shared borrow.
`Endpoints::add_output` and `add_input` allocate or reuse records; scalar output
reuse preserves column slots and lifetime generations. `output` and `input`
return shared records. The binding crate owns subscription and scope indices.

A `Reference` is empty, a generation checked output, or an interned fixed
sequence of child designations. Construct sequences while wiring; switching
between existing designations must allocate nothing. Retaining a designation
does not retain its endpoints. Equality includes the nested designation.

Acceptance: preserve scalar handle size, generation exhaustion, slot reuse and
existing dynamic traces. Fixed slots survive unbinding. No allocation on
steady ticks or warmed fixed REF switches; no scalar type dispatch on reads.

`Scopes`, `Scope`, `Phase` and `Wake` hold graph mailbox storage and route
notifications through owner scopes. `Scopes::alive`, `reserve`, `record_input`,
`record_output`, `wake` and `forward` operate on that storage; teardown policy
remains in Bindings. `ScopeId { index, generation }` identifies its lifetime.
`Reference { output, generation, items }` is empty by default; callers obtain
valid designations from Bindings. Raw record fields are runtime bookkeeping,
not a serialization format or permission to forge cross-store handles.

Shape definitions and observation/binding signatures are listed in the
[bindings card](hgl-bindings.md). `Members<I>` contains live, removed, initial,
changed and epoch tables. `Endpoints` owns input/output vectors and free lists:

```rust
fn output(&self, id: OutputId) -> &Output;
fn input(&self, id: InputId) -> &Input;
fn add_output(&mut self, owner: NodeId, kind: Kind, next_slot: u32, scope: ScopeId) -> (OutputId, bool);
fn add_input(&mut self, owner: NodeId, kind: Kind, active: bool, scope: ScopeId) -> InputId;
```

Stopped scope slots and their retirement time enter `Scopes::retired` and become reusable only at the
next cycle boundary. Retained output references still need their original
scope ancestry to validate forward bindings during the removal cycle.
`Scopes::reclaim(EngineTime, bool, &mut Assemblies)` shares the output expiry boundary, including
independent root runs; entering the retirement cycle itself does not reclaim.

`Kind` is re-exported from `hgl-types::TsType`; its scalar variant is `Ts`.
Descriptions and live endpoints share one recursive shape definition.

`Endpoints::has_peer(input)` excludes a removed-member projection at any
ancestor; retained source handles provide removal-cycle reads, not peering.

`Assemblies` stores interned `(Kind, Vec<Reference>)` records in reusable,
generation-checked slots. `intern(kind, children, scope_claims)`, `get(reference)`,
`retain(reference)`, `release(reference)`, `replace(old, new)` and `counts()`
manage structural ownership; no scalar tick visits this table. `Scope::assemblies`
records construction claims, released by `Scopes::reclaim(now, fresh_run, items)`.
`Reference::same_items(other)` compares both slot and generation for assemblies;
its generation field names the peer or assembly lifetime. `Endpoints::storage_counts`
accepts the scope count and reports the existing four diagnostic counts.

## Prepared membership records

May additionally use hgl-member-table. `Members<I: Copy>` exposes `prepare`,
`prepared_child`, `insert` and `remove`; live, removed and initial records use
reusable `Table` occupancy. `prepared` permanently owns the cold child pool;
`changed` reserves the finite domain capacity. Pool ownership alone confers no
membership or validity.

`Endpoints::reset_prepared(OutputId)` clears descendant membership, timestamps,
validity and held input observations after the removal cycle while retaining
prepared storage and subscriptions. It advances endpoint generations; exhaustion
fails explicitly and never wraps. Pooled future children remain absent.
