# Card: hgl-rust-values

Emit checked hook expressions, statements and nominal global value layouts.
Uses `hgl-source`, `hgl-rust-ir` and `hgl-rust-layouts`; budget 800 source lines. No runtime
execution or third-party dependencies. Graph registration, lifecycle wrapper
emission and eval harness assembly remain in `hgl-rust`.

Public surface: `rust_type`, `scalar_type`, `literal`, `statements`,
`condition_code`, `query`, `global_type`, `global_schema`, `global_markers`,
`owned_type`.
Inputs are frontend-checked IR, not a source validation surface.

Owning tuples retain recursively, including fallible text copies; constructors
evaluate and retain supplied arguments in written order before assembling
declared field order. Prepared aggregate slots represent lexical borrows and
project without copying parents. Retention reads and writes use typed context
APIs, with RHS retention completed before replacement. Canonical nominal marker
identifiers encode the complete qualified specialization source name without hash collisions.

Acceptance: all existing emitted source executions, aggregate global lifecycle
fixtures, retention independence, projection writes and failed replacement.

Ordinary lists lower to owned Vec values and exact List marker types. Indexed
places use checked accessors, and indexed aggregate views use prepared global
slots. Push prepares receiver indices once before item evaluation and retains
items independently, including self-source pushes. Nominal struct markers
stage descendant layout capacity before mutation and implement the runtime's
prepare/install/release/flatten operations. Direct ValueCall expressions use
fallible lexical closures; Yield returns an ordinary value. Endpoint publication
evaluates its value before accessing the mutable runtime context.

`Kind::GeneratorLocal` reads or addresses typed optional node-local storage
created by generator lowering. Reads retain according to the ordinary value
contract; writable field/index operations address the existing owner directly.
`Statement::While` emits an ordinary runtime loop outside generator lowering;
`TimedYield` is emitted exclusively by `hgl-rust-generators`.

Type spelling and nominal marker/schema emission are reexported from
`hgl-rust-layouts`; this crate owns only expression and statement lowering.

Checked temporal emission covers `datetime +/- duration`, `duration + datetime`,
`datetime - datetime`, `duration +/- duration` and unary duration negation.
Operands evaluate once into signed microseconds; checked add/subtract/negate
reports `time arithmetic overflow` without wrapping. Datetime value arithmetic
admits representable pre-epoch results; engine scheduling bounds remain a
separate capability check. This supplies written generator time expressions,
whose failures precede payload evaluation, separately from implicit relative
target resolution after both yield operands. Scaling/division and civil-date
arithmetic remain outside this implementation slice.
