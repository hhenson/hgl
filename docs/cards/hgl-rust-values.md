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

Ordered Delta construction and ObservedLocal aliases are lowered through
hgl-rust-deltas. Structural publication uses typed prepared recursive shapes;
retention materializes sparse observations only at explicit owning boundaries.

Floating modulo evaluates operands once in written order, errors on zero divisors,
and adjusts the direct remainder to the divisor's sign, including signed zero.
It shares constant/wiring semantics without computing a potentially overflowing
or underflowing quotient.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

Re-export hgl-rust-layouts::whole_payload alongside existing layout helpers.

Finite eval recording uses independently prepared destination slots. Direct
publication observations copy through typed temporal tokens; record append fills
an unpublished vacant slot before committing its length. Native field expressions
are evaluated in source order before borrowing record storage. Unrelated ordinary
global mutations retain their existing checked owning API. Direct finite eval
pass-through publishes between typed prepared endpoints without an owning temporary.

Prepared eval transport is selected as one complete path by
`hgl-rust-execution-proof::prepared(plan)`. Unproved plans retain existing ordinary
publication and recording behavior; no per-insertion fallback or hook replay is
introduced. Prepared allocation evidence applies only to the selected finite path.

Supported returned sparse constructors use hgl-rust-direct-deltas to evaluate
scalar operands once, retain constant key aliases cold, and publish directly
into prepared child endpoints. No intermediate delta vectors are constructed.

Struct construction emits supplied payloads in written order, then assembles
all declaration positions, inserting Some/None only at optional fields.
Struct retention delegates to its generated GlobalValue marker so field
presence survives owning boundaries. This introduces no optional read syntax.
