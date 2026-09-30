# Card: hgl-rust

Lower a checked, closed graph plan into Rust source for the existing engine.
This is the Rust backend phase; source linking, inference, diagnostics, handler
normalization and capability admission belong to `hgl-program`. Uses only the
local `hgl-source` crate for shared types and literals. Budget: 700 source lines.
No third-party dependencies or runtime execution dependencies.

Public surface:

- `Value { ty, kind }`, `Value::new(Ty, Kind)`; `Kind::{Literal, Wire, Input,
  Cache, Local, Native, Binary, Unary, Query, Output, Capability, Void}`.
- `Statement::{Let, Return, Call, Add, Assign, For, If}`.
- `Node { name, inputs, result, alarm, start, capability, caches, handlers }`.
- `Native { name, method, throws, args, result }`.
- `Plan { nodes, natives, docs, output, input_length, replay_inputs }`.
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

`Kind::{ReplaySlot,IsPresent,Present}` represents a checked nullable replay
read, its presence test and an extraction justified by frontend flow facts.
`Statement::Exit` ends an evaluation without publication. Nullable values are
owned Rust options; indexing uses the fallible provider read, keeping bounds
errors distinct from absent slots. Capability operations use receiver-first
source spelling; internal runtime method calls do not create source aliases.
