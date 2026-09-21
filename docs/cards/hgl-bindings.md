# Card: hgl-bindings

Status: implemented for the admitted dynamic slice; see [coverage](../runtime-implementation.md).

Own endpoint metadata, lifetime and binding independently of scalar storage.
May use `hgl-types`; `hgl-store` owns the typed columns and delegates metadata
here. This is the same store, not a second runtime. Budget: 1,100 source lines.

The admitted shapes are scalar TS, TSD with i64 keys and scalar children, and
REF to either. Keys in the recorded examples map to distinct i64 values.
Compound children and HGL compiler lowering remain later slices.

## Surface

- `InputId(u32)`, `OutputId(u32)`, `ScopeId`, `Reference`, `Kind` and `BindError`.
  References carry an endpoint generation. Empty and expired references bind
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
binding. Reference designation and target publication are separate events.
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

`InputId` and `OutputId` are public u32 newtypes. `ScopeId` is opaque,
ordered and defaultable (root). `Reference` is opaque, copyable and
comparable; default is empty. `Bindings` is defaultable.

```rust
enum Kind {
    Scalar(ScalarType), Dictionary(ScalarType),
    Reference { scalar: ScalarType, dictionary: bool },
}
enum BindError {
    UnknownInput(InputId), UnknownOutput(OutputId),
    TypeMismatch { input: ScalarType, output: ScalarType },
    ShapeMismatch, AlreadyBound(InputId), BackwardReference,
}
trait Wake { fn wake(&mut self, node: NodeId); }
struct Input { source: Option<OutputId>, slot: u32, kind: Kind, /* private */ }
struct Output {
    slot: u32, kind: Kind, owner: NodeId, scope: ScopeId,
    modified_at: EngineTime, generation: u32, alive: bool, /* private */
}
```

Fields shown are public, exposed through shared borrows only. `Kind::scalar`
and `Bindings` methods:

```rust
fn scalar(self) -> ScalarType;
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
fn removed_input(&self, id: InputId, key: i64) -> Option<InputId>;
fn keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn changed_keys(&self, id: InputId) -> &[i64];
fn added_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn removed_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_;
fn all_valid(&self, id: InputId) -> bool;
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
