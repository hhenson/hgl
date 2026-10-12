# Card: hgl-rust-ir

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `ir` module of `hgl-value-model` (`crates/hgl-value-model/src/ir.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

Checked compiler/backend data boundary. Lives in `crates/hgl-value-model/src/ir.rs`, using the model and `hgl-types`; budget 180
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
Plan.native_images retains inert executable mappings reached by native scalar
types independently of helper calls. The native-images card defines selection
and emission; this metadata never adds a semantic helper binding.

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

`Node: Clone` supports cold suite-level layout collection; node state remains
independent in every generated graph instance.

Kind::Captured(length, Vec<(usize,Value)>) owns a harness result's dense logical
horizon and increasing present slots. It is closed cold data; no silent cells
are allocated. It never enters node runtime storage or ordinary list mutation.

Plan and Native implement Clone for deterministic cold preparation passes. Internal
Unary("family",operand) is a checked ordinary family widening; its cold canonical
result is Construct([(member_index, concrete_value)]), retaining the exact concrete
nominal type. No source cast or temporal projection is introduced.

Plan.ordinary_instantiation marks standalone node registration whose external
caller uses the ordinary graph instantiator. Such a plan has no generated finite
capacity installation phase, even when its source schedule is provably finite.

Value.delta_required records an explicitly written reduced publication requirement at a destination. Kind::IterationInput and Statement::ForItems retain typed child endpoint bindings and the membership/modified selector; all walkers account for the collection and body.

`Value.snapshot` records owning retained Tuple observation representation. It
does not alter canonical `Value.ty`, source identity or explicit delta requirements.
Projection and aliases preserve this emission provenance; private typed optional
slots retain descendant validity independently.

Plan::ordinary_types retains closed harness payload layouts hidden behind Any before startup. It supplies cold descriptor preparation, never runtime type-name discovery.

Finite capacity planning keeps recording rows at the existing cycles-plus-start bound; ordinary global lists preserve initial rows before reserved appends.

Checked IR also retains opaque native values, exact selected native bindings, cold-call phase flags and standalone wiring recipes. All these forms reside in hgl-value-model::ir and are reexported through hgl-eval and semantics.

Internal Query("\0reference", [projected_input]) designates a checked child
endpoint for a compiler-generated graph projection. It reads no scalar payload
and grants no source node access through REF. Its result retains the child's
declared REF schema and lifetime.

Cold subscription preparation follows checked designation publications and
ordinary helper yields through local replacements, branch alternatives, held
output values and projection paths. A REF used only by a guard or metadata
operation supplies no returned target. All possible published targets receive
their typed input views and subscription capacity before the first tick.
Returned designation origins without a cold transport mapping produce a Rust
target diagnostic during finite preparation emission. Collection REF element
selection is one such target gap; a guard-only occurrence does not create a
returned route. Source admission remains separate from this transport check.
