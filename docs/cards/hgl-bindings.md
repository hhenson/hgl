# Card: hgl-bindings

Status: implemented for the dynamic and fixed collection slices; see [coverage](../runtime-implementation.md).

Own endpoint metadata, lifetime and binding independently of scalar storage.
May use `hgl-types` and `hgl-endpoints`; `hgl-store` owns the typed columns and delegates metadata
here. This is the same store, not a second runtime. Budget: 1,100 source lines.

Shapes are recursive: scalar TS, fixed TSL, TSB with named fields, TSD with i64 keys,
and REF. Bundles have no nominal identity yet. Fixed binding pairs children
by position and requires equal ordered shapes; WIR-15 field-name matching
needs a construction-time mapping, including REF rebinds and child boundaries.
HGL collection lowering and growing TSL remain separate slices.

Fixed children occupy stable dense slots. `fixed_input`/`fixed_output` address
positions; `items_reference` builds a reusable child designation. `valid`,
`all_valid`, `last_modified` and `modified` are independent observations.
Assembled validity and time are cached from child events. An invalidated child
retains its event time while its enclosing structure stays valid; a wholly
invalid structure resets descendant observation times. A bind to an invalid
target resets observation time. Whole and item rebinding preserve unchanged
child bindings. Equal REF publication is silent after the first publication.
`input_reference` preserves the current whole or assembled designation.

Retirement visits every descendant, including compound TSD members. Scope
release uses the same lifetime path. Fixed unbinding retains child handles;
releasing the owning scope releases them. Read-only children preserve every
TSB field and TSL position; sparse deltas include only valid modified children.

## Surface

- `InputId(u32)`, `OutputId(u32)`, `ScopeId`, `Reference`, `Kind` and `BindError`.
  References carry an endpoint generation or an interned item designation. Empty and expired peers bind
  to nothing. Scope identity is independent of a graph's local node rank.
- `Wake::wake(NodeId)`: same-scope scheduling; foreign notifications queue in
  their target scope and wake its owner through the enclosing scopes.
- `Bindings`: allocate inputs/outputs, bind/unbind/sample/follow references,
  publish/invalidate endpoints, start a root run and advance the engine
  cycle. `storage_counts` reports retained slots and subscriptions for diagnostics.
- Read-only `Input` and `Output` metadata, including resolved value slot,
  observed time, ownership and type. No public mutable metadata access.
- Dictionary insertion/attachment, removal, live/removed/modified member
  queries and immediate-child validity. A dictionary owns logical membership;
  an attached child graph remains the writer of its output.
- Scope creation/entry/release and queued node/child retrieval. Releasing a
  scope detaches inputs, removes attached outputs from their parent dictionaries
  with a removal notification, and expires those outputs after the current cycle.
  A replacement at the same key is unaffected. Retirement is internal; callers
  remove membership or release its owning scope.

## Rules and cost

TS-1–TS-11, TS-14–TS-23 and GRF-16–GRF-18. Sampled input time is local to the
binding. Invalidating an already-invalid endpoint is silent. Reference designation and target publication are separate events.
Removal retains the child until the next engine cycle; restoration within the
cycle retains its identity. Reusing storage never revives an old reference.

Scalar reads and publications retain indexed access. Only membership/binding
changes may allocate; ordinary value ticks must not. Cleanup visits retired
or changed endpoints, never the whole graph. Notifications cross only the
owning scope chain. Retired slots and scopes are reused; generation overflow
must never revive an old designation.

## Acceptance and mutants

The accepted reference and collection traces are the oracle, including the
owner's expiry ruling. Test passive sampling, equal designation, invalidation,
withdrawal deltas, same-cycle restoration, slot reuse and cross-scope routing.
Each must fail if generation checks, sampling time, ancestor wake propagation,
removed-child retention or detachment is deleted. Test these mutations on an
isolated copy. Allocation tests cover steady ticks after warm-up; churn tests
bound retained endpoint and subscription storage.

## Signatures

`InputId` and `OutputId` are public u32 newtypes. `ScopeId` is ordered and defaultable (root). `Reference` is copyable and
comparable; default is empty. `Bindings` is defaultable.

```rust
enum Kind {
    Ts(ScalarType), Dictionary(Box<Kind>), Reference(Box<Kind>),
    List(Box<Kind>, usize), Bundle(Vec<(String, Kind)>),
}
enum BindError {
    UnknownInput(InputId), UnknownOutput(OutputId),
    TypeMismatch { input: ScalarType, output: ScalarType },
    ShapeMismatch, InvalidReference, AlreadyBound(InputId), BackwardReference,
}
trait Wake { fn wake(&mut self, node: NodeId); }
struct Input { source: Option<OutputId>, slot: u32, kind: Kind, /* private */ }
struct Output {
    slot: u32, kind: Kind, owner: NodeId, scope: ScopeId,
    modified_at: EngineTime, generation: u32, alive: bool, /* private */
}
```

The complete storage records are in [hgl-endpoints](hgl-endpoints.md);
actual runtime records are exposed through shared borrows only. `Kind::scalar`
and `Bindings` methods:

```rust
fn scalar(&self) -> ScalarType; // scalar shapes only
fn len(&self) -> usize;
fn is_empty(&self) -> bool;
fn child(&self, position: usize) -> &Kind;
fn field(&self, name: &str) -> Option<usize>;
fn fixed(&self) -> bool;
fn output(&self, id: OutputId) -> &Output;
fn input(&self, id: InputId) -> &Input;
fn add_output(&mut self, owner: NodeId, kind: Kind, next_slot: u32) -> (OutputId, bool);
fn add_input(&mut self, owner: NodeId, kind: Kind, active: bool) -> InputId;
fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError>;
fn unbind(&mut self, input: InputId);
fn set_active(&mut self, input: InputId, active: bool);
fn last_modified(&self, input: InputId) -> EngineTime;
fn modified(&self, input: InputId, now: EngineTime) -> bool;
fn reference(&self, output: OutputId) -> Reference;
fn resolve(&self, r: Reference) -> Option<OutputId>;
fn reference_value(&self, output: OutputId) -> Reference;
fn sample<W: Wake>( &mut self, input: InputId, r: Reference, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn follow<W: Wake>( &mut self, input: InputId, reference: OutputId, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn set_reference<W: Wake>( &mut self, output: OutputId, r: Reference, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn publish<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W);
fn invalidate<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W);
fn start_run(&mut self);
fn begin_cycle(&mut self, now: EngineTime);
fn child_output(&self, id: OutputId, key: i64) -> Option<OutputId>;
fn child_input(&self, id: InputId, key: i64) -> Option<InputId>;
fn removed_output(&self, id: OutputId, key: i64) -> Option<OutputId>;
fn restorable_output(&self, id: OutputId, key: i64) -> Option<OutputId>;
fn removed_input(&self, id: InputId, key: i64) -> Option<InputId>;
fn keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn changed_keys(&self, id: InputId) -> &[i64];
fn added_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn removed_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn valid(&self, id: InputId) -> bool;
fn all_valid(&self, id: InputId) -> bool;
fn fixed_input(&self, id: InputId, position: usize) -> InputId;
fn fixed_output(&self, id: OutputId, position: usize) -> OutputId;
fn append_fixed(&mut self, parent: OutputId, child: OutputId) -> Result<(), BindError>;
fn items_reference(&mut self, kind: Kind, children: Vec<Reference>) -> Result<Reference, BindError>;
fn insert<W: Wake>( &mut self, dict: OutputId, key: i64, child: OutputId, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn remove<W: Wake>(&mut self, dict: OutputId, key: i64, now: EngineTime, wake: &mut W);
fn input_reference(&self, input: InputId) -> Reference;
fn storage_counts(&self) -> [usize; 4];
fn enter_scope(&mut self, scope: ScopeId) -> ScopeId;
fn scope(&self) -> ScopeId;
fn child_scope(&mut self, owner: NodeId) -> ScopeId;
fn reserve_scope(&mut self, scope: ScopeId, nodes: usize);
fn take_wake(&mut self, scope: ScopeId) -> Option<NodeId>;
fn take_child(&mut self, owner: NodeId) -> Option<ScopeId>;
fn release_scope<W: Wake>(&mut self, scope: ScopeId, now: EngineTime, wake: &mut W);
```

A retained removed output from a stopped scope remains readable for its removal
cycle but cannot be reattached. `restorable_output` admits only a live writer;
restoration otherwise allocates fresh parent-owned storage. Insertion rejects
dead endpoints and stopped scopes before changing membership. Compound
attachment uses a peer `Reference`, checking generation even after slot reuse.

`has_peer(input)` reports a current binding. A retained removed member and its
fixed descendants have no peer, though their source handles keep removal-cycle
values readable (TS-11). This diagnostic follows parent membership; storage
retention alone does not establish peering.

Assembly storage is generation checked and reusable. `assembly_counts()` reports
retained slots and live records. Construction scopes, current input designations,
REF outputs and enclosing assemblies retain records; copying a handle does not.
Scope claims end at the next cycle after teardown. Retaining a record never
retains its target endpoints. Rebinding, unbinding and output expiry release
old claims. Interning uses a hash index rather than a scan of historical records.

`bind_designation(InputId, OutputId) -> Result<(), BindError>` captures a
rank-valid source for a REF parameter without a value subscription. A live
identity is valid before its payload; retired generations read empty.
`input_reference` reads a REF input's designation, or an ordinary input's peer.
Set membership reuses dictionary removal/delta bookkeeping with occupancy
children. No set element equality checks are added to scalar publication.
