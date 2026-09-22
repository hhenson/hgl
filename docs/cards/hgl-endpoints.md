# Card: hgl-endpoints

Status: implemented for the fixed collection slice.

Own endpoint records, recursive shapes and reusable slot allocation. No
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
