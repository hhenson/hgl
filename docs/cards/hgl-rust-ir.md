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
