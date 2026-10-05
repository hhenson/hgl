# Card: hgl-rust-ir

Checked compiler/backend data boundary. Uses only `hgl-source`; budget 180
source lines. No runtime dependencies or third-party crates.

Surface: `Value { ty, kind }`, `Value::new`; `Kind`, `Statement`, `Node`,
`Native`, and `Plan` retain the checked plan surface reexported by `hgl-rust`.
Their fields and scalar/capability forms are specified in that card.
`Kind::Construct(Vec<(usize, Value)>)` holds supplied ordinary struct arguments
in written source order, paired with their checked declared-field indices. `Kind::Field(Box<Value>, usize)` projects a checked field index.
`Ty::Struct` preserves nominal identity and field types; these values are
ordinary values, never temporal endpoints.

The frontend checks type identity, access authority, indices, and retention
before emission. This crate performs no source checking and selects no runtime
behavior. Construction and projection preserve the ordinary value-mutability
contract; aggregate views explicitly retain their provenance.

Acceptance: program source and emitted-executable tests for required-field
struct construction, nested independent copies, mutable assignment and
read-only diagnostics; all existing scalar and capability regressions.

`Kind::BorrowedLocal(local, entry, writable)` records a lexical aggregate
view's local identity, prepared entry identity and access authority.
`Statement::Borrow(local, initializer, writable)` binds a typed get or an admitted
readonly alias without copying the payload. Aggregate `GlobalGet` appears only
in this binding form; scalar gets remain owned. `Field` preserves a borrowed
root's prepared projection. `Node::globals` includes exact ordinary struct types.

`Kind::{List, Index, Length, Push}` carries checked ordinary construction,
projection, observation and content mutation. `ValueCall(arguments, body)`
represents direct ordinary execution; `Statement::Yield` returns its value
without publishing an endpoint. Statements are cloneable so direct bodies can
be retained in expressions. `Kind::Configuration` reads a readonly retained
node configuration field, populated by `Node::configuration`.
`Plan::construction_error` carries a deterministic wiring operation failure to
the construction boundary; source checking errors never use that field.

`Kind::WiringFailure` carries a typed unavailable result after an operation
fails during deterministic wiring. It lets subsequent source statements retain
normal type checking without manufacturing a successful payload. A plan with
such a failure emits only the construction failure, not executable node bodies.

`Node::generator` holds an optional source body, mutually exclusive with normal
lifecycle hooks and handlers. `Statement::TimedYield(time, payload)` evaluates
time then payload before target resolution; it is distinct from ordinary
`Yield`. `Statement::While(condition, body)` preserves checked runtime loops.
Generator body local identifiers are unique across nested lexical blocks.
`Kind::GeneratorLocal(id)` is backend-only: generator lowering replaces its
outer lexical local uses with owned node storage, leaving value-call body
locals lexical. No borrowed global view is admitted into generator storage.

`Kind::Delta(Vec<DeltaEntry>)` retains ordered structural delta constructor
parts. `DeltaEntry::{Add(Value),Remove(Value),Keyed(Value,Value),Child(i64,Value)}`
retains typed scalar keys separately from fixed collection indices; Value.ty is exact Ty::Delta(origin), so each
child's representation is determined before runtime. No shape registry or
runtime type test is required. Every key and child expression is evaluated once and
retained before the next part, with a map key preceding its payload. `Kind::ObservedLocal(id)` preserves a readonly
evaluation-local structural delta observation; it is not an owning copy.
Direct structural Query(delta_value) and observation aliases retain endpoint
observation identity until an admitted owning retention/publication boundary.

`Value::closed()` recognizes fully retained literal/list/struct/delta data and
void; it rejects every operational or borrowed IR form, including wiring
failure. Ordinary evaluation shares this classifier across its phase boundary.

Plan recording metadata carries its ordinary key and exact retained list type.
The eval run owner reads that prepared entry after stop; no node-private capture
buffer is needed. Source replay configuration consists of ordinary value data.

## Temporal scalar preparation

Kind::TemporalLiteral(TemporalLiteral) retains contextual scalar construction;
Kind::Prepared(usize) references a statically typed cold configuration binding.
Neither is a closed compile-time value; Prepared is never a node-hook operand.

DeltaEntry::operands visits retained key/payload expressions in source order;
try_map maps those same expressions fallibly, preserving indexed versus keyed
identity. closed() traverses keys as well as payloads: contextual recipes are
not closed merely because they occur in membership/removal data.

DeltaEntry::operands_mut exposes the same source-order traversal for cold IR
rewrites, retaining typed keys alongside their payloads.
