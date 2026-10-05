# Card: hgl-rust

Lower a checked, closed graph plan into Rust source for the existing engine.
This is the Rust backend phase; source linking, inference, diagnostics, handler
normalization and capability admission belong to `hgl-program`. Uses the
local `hgl-source`, `hgl-rust-ir`, `hgl-rust-values` and `hgl-rust-generators` crates for shared types and literals. Budget: 700 source lines.
No third-party dependencies or runtime execution dependencies.

Public surface:

- `Value { ty, kind }`, `Value::new(Ty, Kind)`; `Kind::{Literal, Wire, Input,
  Cache, Local, MutableLocal, Native, Binary, Unary, Query, Output, Capability, Void}`.
- `Statement::{Let, Var, Return, Call, Assign, For, If}`.
- `Node { name, inputs, result, alarm, start, caches, handlers }`.
- `Native { name, method, throws, args, result }`.
- `Plan { nodes, natives, docs, output, recording, input_length }`.
- `emit(&Plan) -> String` and
  `emit_test_body(&Plan, Option<&[Option<Literal>]>) -> String`.

The IR is the compiler/backend boundary, not a user-input validation API.
Its public fields let the frontend assemble a plan after checking source types,
phases, endpoint proofs and graph wiring. Values, node/native indices, method
names and statement shapes must satisfy those checks before emission. Frontend
markers (`Wire`, `Capability`, `Void`) are not executable payload values.
`Input` retains the formal signal marker even when its storage type is scalar.
No additional language admission or operator selection occurs in this crate.

Emission owns native trait signatures, typed node fields and lifecycle hooks,
graph registration/construction and test execution bodies. Eval storage is fresh
and owned per constructed graph; typed fields and source-checked wiring retain
the existing binding guarantees. The backend emits the supplied start/evaluation
statements; it does not select runtime behavior by replay/record operator name.
The generated Rust references runtime crates, but the emitter does not link or
execute them. `hgl-program` remains the source-facing compilation facade.

Acceptance: byte-identical emitted Rust across the extraction for const/debug,
compiler eval regressions and the pinned standard suite; existing program tests
compile and execute emitted Rust in debug/release. No HGL behavior changes or
structural delta admission are part of this phase extraction.

`Kind::{IsPresent,Present}` retains contextual presence-test and extraction IR.
`Statement::Exit` ends an evaluation without publication. Ordinary list reads
are checked and fallible. Capability operations use receiver-first source
spelling; internal runtime method calls do not create source aliases.

`Node { global_state, globals, stop, .. }` records a run-wide shared-store requirement
and checked stop-hook statements. The `globals` key/type pairs describe prepared entries; `Kind::GlobalGet` and
`Kind::GlobalSet` address them by node-local index. Emission declares typed
`Global<T>` handles, construction-only `NodeType::global_entries` metadata, and
`Ports::global` binding. Hook calls use those handles without key lookup or type
dispatch. Values remain run-owned. Construction validates provisioning and all
entry requirements before any start hook.

`MutableLocal` and `Statement::Var` retain writable owned scalar binding identity.
Assignments target the checked binding; compound addition lowers through the
ordinary typed binary expression and assignment paths. Read-only locals remain
`Local`. Every generated eval provisions its fresh Store before construction.

The checked IR is defined by `hgl-rust-ir` and reexported unchanged here.
`Kind::{Construct,Field}` lower ordinary required-field struct locals to owned
Rust tuples. Field access copies only the selected value, not its parent.
Assignment addresses the writable local or nested field directly; its owned
right-hand value is evaluated before replacing the destination. Canonical
nominal identity remains in the checked IR. Structs are not admitted as ports,
native arguments, cache entries or recording containers.

Owning local/field retention recursively copies tuple fields, using the scalar
provider's fallible text copy. Nullable scalar locals retain independent copies
as before; text payloads use the same fallible copying path.

Ordinary constructor arguments evaluate exactly once in source order; each
argument's owned result is retained before the next argument executes.
Assembly moves the retained temporaries into their declared-field positions.
Failure stops later arguments without exposing a completed value, following
`struct-constructor-order.md`. This slice does not admit default fields.

Hook expression and statement emission is delegated to `hgl-rust-values`.
Required-field aggregate entries emit nominal `GlobalValue` markers whose
value representations are owned tuples and prepared field layouts are typed
`ValueSlot` tuples. Entry descriptors retain nominal identity and field schema.
Aggregate borrow bindings use `global_state().borrow` once to check presence and bind
a prepared slot; field projections select slot fields without payload copying.
Reads at explicit retention boundaries use `global_state().read`; borrowed field and
whole-value assignment evaluates its owned RHS before `global_state().write`.

Ordinary configuration fields use owned Rust representations and are retained
once during node construction. Construction errors from deterministic wiring
are returned before graph start. Direct value-function bodies remain local
fallible calls and do not receive node scheduling.

Generator lifecycle emission delegates to `hgl-rust-generators`: typed local
storage and pending output are constructed per node, start resets and arms the
first evaluation, and evaluation executes the checked resume machine. Ordinary
handlers retain their existing path. Generator selection uses checked IR only,
never a source operator name or native role.

Structural ports store prepared hgl-shapes tokens, validated at node build.
Generated delta shape application/extraction is emitted by hgl-rust-deltas.

Eval prepares its typed ordinary recording binding before graph start and reads
an independent owned list after stop. It converts timed scalar entries to the
existing dense observation comparison while retaining the separate input horizon.
It does not inspect a recorder node's private storage or inject replay data.

`emit_test_body(plan, expected: Option<&[Option<Value>]>)` materializes checked
closed expected values after stop and uses shape-specific sparse comparison.
Recorded construction errors return before generating references to absent
node/layout declarations. Expected emission is a fallible owning boundary.

## Prepared test execution

Expose emit_prepared_test_body(&Plan) -> String beside emit_test_body. It emits
a generated run(PreparedEval) -> Result<CapturedEval, String> adapter that
binds exact native prepared configurations before graph construction and encodes
independent captures after teardown. May use hgl-rust-preparation for cold typed
conversion; existing emit_test_body remains available. Prepared values never
reach node hooks or control graph topology.

`shared_layouts(&[Plan])` emits each exact nominal layout once across a test
suite. `emit_shared(&Plan)` emits nodes against those enclosing markers; standalone
`emit` retains self-contained output. Graph instances and payload stores remain
independent. Sharing generated type definitions bounds compiler memory growth
without changing execution or reducing test cases.

Finite evaluation emits graph-local cold capacity maxima from already materialized
configuration values and proved direct generator arrival counts. Before binding globals it allocates record slots, then after
instantiation prepares scalar, atomic, and every finite keyed descendant output.
Configuration source arenas own independent typed slots. Captures are extracted as
owning values after simulation; generated hook recording copies into reserved slots.

Emission normalizes literal-only family publications through hgl-rust-families
before node construction, prepared adapters and execution-body generation. Native
provider/call expressions are never moved by this narrow preparation pass.
Prepared eval transport is selected as one complete path by
`hgl-rust-execution-proof::prepared(plan)`. Unproved plans retain existing ordinary
publication and recording behavior; no per-insertion fallback or hook replay is
introduced. Prepared allocation evidence applies only to the selected finite path.

Entry points normalize immutable owning scalar aliases and literal delta operands
with hgl-rust-direct-deltas before selecting the whole prepared adapter. The
normalization is idempotent and never executes user code or provider recipes.

The standalone emit entry point marks ordinary_instantiation on its backend clone.
Its public register/graph-description API leaves instantiation to the caller and
does not install finite evaluation capacity. Shared eval adapters retain their
explicit preparation phase and allocation-free proved transport.
