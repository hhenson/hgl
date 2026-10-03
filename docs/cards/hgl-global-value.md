# Card: hgl-global-value

Typed representations of ordinary required-field nominal structs whose finite
fields recursively contain the eight supported scalars. Uses `hgl-types` and
`hgl-columns`; budget 200 lines. No third-party dependencies or unsafe code.

`GlobalValue` names a generated nominal marker's `Value` (ordinary owning Rust
representation) and Copy `Slots` (typed field handles). Its methods are
`schema() -> OrdinaryType`, `slots(&mut &[usize]) -> Slots`,
`retain(&Value) -> Result<Value, Box<NodeError>>`,
`read(&Columns, Slots) -> Result<Value, Box<NodeError>>`, and
`commit(&mut Columns, Slots, Value)`. Scalar types implement this trait directly.
Generated struct implementations delegate retention/read/commit recursively,
retain all fields before commit, and move each owned field exactly once.
`Columns` is re-exported for generated implementations.

`ValueSlot<T: GlobalValue>` is an opaque Copy/Debug typed handle with
`bind(&mut &[usize]) -> Self` for construction, `fields() -> T::Slots` for
prepared projection, `read(&Columns)` and `commit(&mut Columns, T::Value)`.
Slots consume the finite schema's scalar positions in declaration order.
`allocate(&OrdinaryType, &mut Columns, &mut Vec<usize>)` prepares those positions
before graph start; schema inspection and name/type reconciliation never run
inside hooks. Marker implementations are compiler/native provider contracts.

Rules: ADR 0016 exact nominal entry types and independently retained replacement;
value-mutability lexical aggregate access and VAL-17 independent ownership.
Borrow/projection itself copies only handles and never payloads. Required fields
share the root entry's presence. Strings may allocate when explicitly retained.
The compiler checks lexical access conflicts, including helper effects.

Acceptance lives in hgl-store and hgl-describe: nested primitives, nominal and
scalar conflicts before start, absent root, borrowed read/write/replacement,
independent retention and failed retention preserving every previous field.
Mutants: erase nominal identity; copy payload at borrow; commit first field before
later retention fails; project a child to the wrong typed slot; replace through a
borrow without updating the original root.
