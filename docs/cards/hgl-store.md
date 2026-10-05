# Card: hgl-store

## Purpose

Where time-series live and how a tick travels. The store owns every value,
every last-modified time and every binding in a run. It knows nothing about
nodes beyond an id to wake.

## May use

`hgl-types`, `hgl-bindings`, `hgl-columns`, `hgl-shapes`.

The ordinary global-value facility also uses `hgl-global`, independently of
time-series columns. Store re-exports its typed `Global<T>` handle.

`global_state().provision()` enables the run's store; `globals().provisioned()`
reports availability. `global_state().bind<T>(key)` and
`global_state().prepare(key, OrdinaryType)`
resolve exact types during owner configuration/graph construction.
`globals().get(Global<T>) -> Result<T::Value, Box<NodeError>>` and
`global_state().set(Global<T>, &T::Value) -> NodeResult` use only typed slots and presence on
the hook path. Bindings do not initialize values. Owned scalar copies are
independent of later sets; String copying can allocate/fail. These methods
neither publish nor schedule. Nested graph scopes share the same facility;
independent runs use distinct Stores. Owner binding/extraction is outside hooks.

## The layout

Scalar values remain in typed columns, addressed by dense indices. Endpoint
metadata is owned by `hgl-bindings`: input source/slot, local sample time,
output time/generation, subscriptions, dictionary membership and graph scope.
Inputs cache the value slot. Output handles carry slot identity and generation
in eight bytes, so a retained writing handle cannot address a reused endpoint.
Both crates are safe Rust; no stored reference is a borrowed pointer.

## Surface

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct OutputId(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct InputId(pub u32);

/// A scalar the store has a column for. Sealed: implementations live in hgl-columns.
/// The seal is a supertrait in a private module (`+ columns::Column`), which
/// another crate cannot implement. Its methods can still be reached through a
/// `T: Scalar` bound, but only on a `Columns` of the caller's own making,
/// never a store's.
pub trait Scalar: Clone + PartialEq + std::fmt::Debug + 'static {
    const TYPE: ScalarType;
    fn into_value(self) -> ScalarValue;
    fn from_value(value: ScalarValue) -> Option<Self>;
}

/// A node's handle to its own `TS<T>` output. Eight bytes.
#[derive(Debug, Clone, Copy)] pub struct Out<T: Scalar> { /* OutputId, generation */ }
/// A node's handle to one of its `TS<T>` inputs. Four bytes.
#[derive(Debug, Clone, Copy)] pub struct In<T: Scalar> { /* InputId */ }
impl<T: Scalar> Out<T> { pub fn id(self) -> OutputId; }
impl<T: Scalar> In<T>  { pub fn id(self) -> InputId; }

/// Who is told that a node must be evaluated in this cycle. The kernel's
/// schedule implements it. Generic, not `dyn`: a wake is inlined. Must be
/// idempotent: a node is woken twice in one cycle when two of its active
/// inputs tick, or when one input is re-bound to an output that then ticks.
/// (An output notifies once per cycle, TS-6; that a node so woken is
/// evaluated once, GRF-16, is the schedule's to keep.)
pub trait Wake { fn wake(&mut self, node: NodeId); }

#[derive(Debug, Default)]
pub struct Store { /* private */ }

impl Store {
    pub fn new() -> Self;

    // --- instantiation: may allocate ---
    pub fn add_output<T: Scalar>(&mut self, owner: NodeId) -> Out<T>;
    pub fn add_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> In<T>;
    /// By id, for the builder, which knows types only at run time.
    /// Errors: either id unknown; the types differ; the input already bound.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError>;
    pub fn unbind(&mut self, input: InputId);
    pub fn input_type(&self, input: InputId) -> ScalarType;
    pub fn output_type(&self, output: OutputId) -> ScalarType;

    // --- per tick: never allocates, never searches ---
    /// The bound output's value. Debug builds assert the input is valid.
    pub fn get<T: Scalar>(&self, input: In<T>) -> T;
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool;
    /// `valid`, by id: what the kernel asks of a node's required inputs
    /// before it calls `eval` (NOD-2). The same two loads and a compare.
    pub fn input_valid(&self, input: InputId) -> bool;
    pub fn modified<T: Scalar>(&self, input: In<T>, now: EngineTime) -> bool;
    pub fn last_modified<T: Scalar>(&self, input: In<T>) -> EngineTime;
    pub fn set_active<T: Scalar>(&mut self, input: In<T>, active: bool);

    /// Write the value. If this is the output's first write at `now`, stamp
    /// it and wake the owner of every active watcher; a later write at `now`
    /// only changes the value (TS-6). `writer` is the node being
    /// evaluated; debug builds assert it owns `output` (TS-21). Release builds
    /// do not read it.
    pub fn set<T: Scalar, W: Wake>(&mut self, output: Out<T>, value: T, now: EngineTime,
                                   writer: NodeId, wake: &mut W);
    /// A node reading its own output (INJ-8): `None` until it has ticked.
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T>;

    // --- the erased path: tests and tools, never a node's eval ---
    pub fn output_value_erased(&self, output: OutputId) -> Option<ScalarValue>;
    pub fn output_modified(&self, output: OutputId, now: EngineTime) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindError { UnknownInput(InputId), UnknownOutput(OutputId),
                     TypeMismatch { input: ScalarType, output: ScalarType }, AlreadyBound(InputId) }
```

## Rules

TS-1 (valid and modified are read from the last modified time), TS-2 (an
invalid input has no value — `get` asserts in debug; callers check `valid`),
TS-3, TS-6 (an output notifies on its first write in a cycle only), TS-8 (a
passive input never wakes; `set_active` changes no reading), TS-14 (a plain
bind notifies nothing), TS-21, TS-22 (only `set` writes; `get` returns a
copy), NOD-7, GRF-16, GRF-17.

*Revised after the first build (2026-09-19): the type columns were missing
from the layout; `set` could not assert its writer; `Wake` must be idempotent.
A new output's slot holds the scalar's default until its first `set`, which
nothing can observe. An id that does not exist is a bug in the caller and
panics by indexing; only `bind`, which the builder calls with ids it did not
make, returns an error. TS-3's "invalidation returns it to never" has no
entry point here yet. `modified` is read from the output's `modified_at`.
A rule whose proof is a signature has no test that can fail — TS-22's `get`
returns a `Copy` value; TS-14's `bind` has no `Wake` — and the test's comment
says that instead of claiming to check it. `output_modified` asserts about
`NEVER` as `modified` does.
**The per-input `notified_at` column is gone.** A reviewer deleted it and
every test passed; the reference settles why: hgraph's C++ keeps
once-per-cycle on the output (a write at a time already stamped returns
before the observers are walked) and lets the node's schedule slot be
idempotent. Only the Python runtime stamps inputs. So `set` on a time already
stamped is a value write and nothing else, and the walk reads two columns per
watcher, not three. Making an input active never wakes its node, in C++, even
if its source has ticked; a wiring-time bind never notifies — which is why
`set_active` and `bind` take no `Wake`. A run-time rebind does notify, and
arrives with references.
Revised again when the kernel was built: the kernel holds a node's required
inputs as ids and had no way to ask whether one is valid — `input_valid`.*

## Speed

- The eight per-tick methods — `get`, `valid`, `input_valid`, `modified`,
  `last_modified`, `set_active`, `set`, `output_value` — contain no allocation, no hashing, no
  `dyn`, and no loop except `set`'s walk of the watcher list.
- `watchers` may allocate when a binding is made, never when a tick is sent.
- No `unsafe`. If a benchmark misses its 5%, the first things to try are
  `get_unchecked` behind debug assertions, then raw pointers cached at bind
  time over chunks that never move — in that order, each justified by a
  measurement in the commit that adds it.
- Debug assertions check what release trusts, each naming its rule: a
  handle's type matches its entry (a handle from another store of the same
  type is not caught — it names the wrong entry); `set`'s `writer` owns the
  output (TS-21); an input is valid before it is read (TS-2); time never
  runs backwards (TS-3); `NEVER` is never an evaluation time, for `set`,
  `modified` and `output_modified`.

## Budget

500 lines.

## Done when

Unit tests, each naming its rule: a tick stamps and wakes; a second `set` in
one cycle overwrites and wakes nobody again; a passive watcher is not woken
and still reads the value and `modified`; an unbound input is not valid;
`bind` rejects a type mismatch; after warm-up, 10,000 `set` + `get` rounds
make zero allocations (with `hgl-alloc-count`).

## Mutants

Each must make a test fail.

- `set` wakes passive watchers as well as active ones.
- `set` walks the watchers on every write, not only the first at a time.
- A second write at one time does not change the value.
- A watcher bound between two writes of one cycle is woken by the second.
- (That `bind` wakes nobody is held by its signature, which has no `Wake`:
  say so where TS-14 is tested, and do not assert on a `Wake` that `bind`
  cannot reach. The same holds for `set_active` and TS-8.)
- A refused `bind` — either reason — changes something before it returns:
  the watcher list, `source`, or `source_slot`.
- `modified` reads false for an input bound after its source's tick in the
  same cycle, or true for one unbound after it.
- `output_value` indexes the column by the output's id, not its slot.
- `set_active` always writes input 0.
- The first write at `MIN_START` wakes nobody.
- Any one of the Speed section's debug assertions is deleted. (Debug
  builds only.)
- `valid` ignores `NEVER` (a bound input is always valid).
- `modified` compares with `>=` instead of `==`.
- `unbind` leaves the input reading its old source.
- `bind` accepts a type mismatch.
- `input_valid` is true for any bound input, ticked or not.

## Dynamic slice

Status: implementation contract. The original scalar methods and budget stay.
Endpoint metadata moves to [hgl-bindings](hgl-bindings.md); Store keeps typed
columns and exposes its read-only `bindings()` for logical observations.

New surface: `DictOut<T>`/`DictIn<T>` with `id`; dictionary allocation,
`child`/`removed_child`, `get_or_create`, `attach`, `remove`; `invalidate`,
`reference`, `sample`, `add_reference`, `follow`, `set_reference`; and
`begin_cycle` and `start_run`. Root startup begins independent cycle
bookkeeping without resetting output values; child startup shares its parent
clock. Dictionary keys are i64 in this admitted slice. No value is
copied when attaching a child graph's output. References carry generations.

Graph integration adds `scope`, `enter_scope`, `child_scope`, `reserve_scope`,
`release_scope`, `take_wake` and `take_child`. These operate on scope identity,
not local rank. Existing scalar handles retain their size and indexed reads.

The new handles are `Copy`; both have `fn id(self)` returning their
corresponding endpoint id. `Reference` and `ScopeId` are re-exported.
`output_value_erased` returns `None` for non-scalar shapes; their observations
come from `bindings()`. Raw endpoint ids and input handles are local to their owning graph lifetime;
retain a `Reference` when a designation must outlive that graph.

```rust
fn bindings(&self) -> &Bindings;
fn start_run(&mut self);
fn begin_cycle(&mut self, now: EngineTime);
fn invalidate<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W);
fn reference(&self, output: OutputId) -> Reference;
fn sample<W: Wake>( &mut self, input: InputId, r: Reference, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn add_reference( &mut self, owner: NodeId, scalar: ScalarType, dictionary: bool) -> OutputId;
fn follow<W: Wake>( &mut self, input: InputId, reference: OutputId, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn set_reference<W: Wake>( &mut self, output: OutputId, r: Reference, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn add_dictionary<T: Scalar>(&mut self, owner: NodeId) -> DictOut<T>;
fn add_dictionary_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> DictIn<T>;
fn child<T: Scalar>(&self, input: DictIn<T>, key: i64) -> Option<In<T>>;
fn removed_child<T: Scalar>(&self, input: DictIn<T>, key: i64) -> Option<In<T>>;
fn get_or_create<T: Scalar, W: Wake>( &mut self, dict: DictOut<T>, key: i64, now: EngineTime, wake: &mut W) -> Out<T>;
fn attach<T: Scalar, W: Wake>( &mut self, dict: DictOut<T>, key: i64, child: Out<T>, now: EngineTime, wake: &mut W) -> Result<(), BindError>;
fn remove<T: Scalar, W: Wake>( &mut self, dict: DictOut<T>, key: i64, now: EngineTime, wake: &mut W);
fn scope(&self) -> ScopeId;
fn enter_scope(&mut self, scope: ScopeId) -> ScopeId;
fn child_scope(&mut self, owner: NodeId) -> ScopeId;
fn reserve_scope(&mut self, scope: ScopeId, nodes: usize);
fn release_scope<W: Wake>(&mut self, scope: ScopeId, now: EngineTime, wake: &mut W);
fn take_wake(&mut self, scope: ScopeId) -> Option<NodeId>;
fn take_child(&mut self, owner: NodeId) -> Option<ScopeId>;
```

## Fixed collection slice

`add_shaped_output(owner, Kind)` and `add_shaped_input(owner, Kind, active)`
construct recursive endpoints. `scalar_output<T>(id)` / `scalar_input<T>(id)`
validate leaf shapes once and return the existing typed handles. All steady
scalar access retains its column path. `items_reference(kind, children)`
constructs a reusable designation; `attach_shaped` and `get_or_create_shaped`
allow compound TSD members. `remove_shaped` removes them by key. Observation
uses `bindings()` and dense child positions, retaining invalid fields as nil;
field names are resolved at construction, never per tick.

`attach_shaped(dict: OutputId, key: i64, child: Reference, now, wake)` requires
a generation-checked peer designation; empty, expired and assembled values
return `InvalidReference`. `get_or_create_shaped` replaces a removed stopped
writer with fresh storage rather than reviving its retired descendants.

## Owned scalar payloads and reference capture

`Store::get_ref<T: Scalar>(In<T>) -> &T` borrows a valid payload;
`output_ref<T: Scalar>(Out<T>) -> Option<&T>` also checks output generation. `get` clones
owned values when a caller needs ownership; numeric reads remain copies.
`bind_designation(InputId, OutputId)` captures a compatible reference input's
source identity without subscribing to value ticks. Generation checks prevent
retired members from reappearing after slot reuse. `get_or_create_shaped`
also admits sets, using boolean occupancy children.

Ordinary globals also support finite required-field nominal structs via
`GlobalValue` markers (re-exported with `ValueSlot` and `ValueColumns` from the typed
storage representation). GlobalState's `bind`, `get` and `set` use
`T: GlobalValue`, with owning payload `T::Value`; all scalar callers are unchanged.
`prepare` takes `OrdinaryType`. `global_state().borrow(Global<T>)` checks the single
root presence and returns `ValueSlot<T>` without copying the entry.
`global_state().read(ValueSlot<T>)` returns an independently owned `T::Value`;
`global_state().write(ValueSlot<T>, &T::Value)` retains fully before replacing a borrowed
root or projected required field. Slots stay internal to the generated lexical
access discipline. No runtime borrow registry or per-hook schema inspection.

`Store::global_state() -> &mut GlobalState` provides the same run-owned capability
for list operations and owner configuration/extraction. `GlobalState`, `List`,
`Capacity`, `Layouts`, `ValueColumns`, and the ordinary list helpers are re-exported
for generated native value code. Ordinary ValueColumns use a reusable typed arena;
temporal Columns and endpoint storage remain separate. The compiler restricts
hook calls to already-prepared typed accesses.

Prepared shape input/output tokens from hgl-shapes can be converted to scalar
handles without repeating type checks. `prepared_input<T>(Input<T>) -> In<T>`
and `prepared_output<T>(Output<T>) -> Out<T>` preserve the static shape proof.
`add_prepared_output(owner, kind, children)` attaches statically allocated fixed
children. `get_or_create_with(dict,key,now,wake,create)` uses a compile-time chosen
child factory only when no live/restorable child exists. Existing dynamic
construction entry points remain available; no payload dispatch is introduced.

Atomic ordinary payloads use hgl-atomic's prepared Arena (an allowed dependency).
`add_atomic_output<T: GlobalValue>(NodeId) -> OutputId` prepares the exact
root layout before publication; add_shaped_output handles the same cold path. `atomic_borrow<T>(Input<Atomic<T>>) ->
NodeResult<ValueSlot<T>>`, `atomic_get<T>(Input<Atomic<T>>) -> NodeResult<T::Value>`
and `atomic_values() -> &ValueColumns` provide prepared borrowing and explicit
owning reads. `set_atomic<T, W: Wake>(Output<Atomic<T>>, T::Value, EngineTime,
&mut W) -> NodeResult` commits the prepared complete replacement, then marks
and wakes the existing binding. Validity and modification remain in Bindings.

`globals() -> &GlobalState` provides immutable access to the same run-owned store.

## Finite collection preparation

May additionally use hgl-keys and hgl-store-build. `Key` and `Keys` are
re-exported; `Store::keys: Keys` retains exact typed ordinary key values before
instantiation. Runtime endpoints use opaque stable i64 domain tokens, without
coercing the ordinary key identity.

`prepare_collection(root, &[i64], FnMut(&mut Store,NodeId)->OutputId)` cold
allocates every possible child through the statically selected factory. Factories
may recursively prepare nested collections. `prepare_collection_inputs()` runs
once after complete wiring and before target startup, preparing all projections
and cycle work capacities. Both follow the existing infallible construction
contract; invalid domain/shape assumptions fail explicitly.

Prepared children are absent and invalid until insertion. First insertion,
removal, subsequent reinsertion and projection synchronization reuse storage.
The removal cycle retains values; its next boundary resets descendant validity,
nested membership and generations, so old references/writers cannot revive
stale fields. Active membership traversal ignores never-used domain slots.
Generation exhaustion fails explicitly rather than wrapping a designation.

The finite prepared evaluation profile has whole-cycle zero-allocation tests,
including nested maps, partial bundle fields, held references and owning String,
provider ZoneId and ZonedTime keys. Existing unprepared dynamic construction
and runtime rebinding remain outside this finite-domain allocation guarantee.

prepared()->PreparedStorage returns disjoint runtime borrows for cold setup and
finite prepared copying. PreparedStorage, PreparedTick, Observation, PreparedValue,
ListBounds, append_slot and commit_append are reexported. Existing dynamic APIs
remain available; finite generated evaluation uses independently reserved storage
and never replaces that capacity with source-owned aliases.

Out::generation()->u32 exposes the original writing-token generation for the
prepared publication facade; it never substitutes the current endpoint generation.

Re-exports `hgl_optional::Optional` for generated concrete struct field markers.
Optional snapshot semantics are implemented; complete finite evaluation
allocation freedom remains guarded by the new generated allocation test and
must not be inferred from isolated prepared scalar tests.
