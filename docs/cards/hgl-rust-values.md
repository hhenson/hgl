# Card: hgl-rust-values

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `values` module of `hgl-rust` (`crates/hgl-rust/src/values.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

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

Struct construction emits supplied payloads in written order, then assembles
all declaration positions, inserting Some/None only at optional fields.
Struct retention delegates to its generated GlobalValue marker so field
presence survives owning boundaries. This introduces no optional read syntax.

Internal family widening preserves tuple-valued expression parentheses and exact
discriminators. Returning immutable configuration payloads uses typed prepared
slot publication, avoiding an owning clone in the evaluation hook.
Canonical borrowed family projections delegate to collections::family_field,
including families nested in owning Struct and List storage. Reads and writes
address the active member's common field; independent retention preserves the
family payload when another owner is mutated. Index expressions evaluate once
before dispatch, and failed access stops later operands.
Native scalar-only family locals and staged helper arguments retain an exact native type annotation so
inactive member positions preserve their declared types without canonical
owning storage. Scalar construction, projection and atomic publication use the
existing stack-value transport, preserving tags and copies across mutation.

Complete ordinary set/map construction delegates key-first validation to hgl-rust-collections; retention uses the exact GlobalValue marker and comparison is recursively unordered.
Prepared eval transport is selected as one complete path by
`hgl-rust-execution-proof::prepared(plan)`. Unproved plans retain existing ordinary
publication and recording behavior; no per-insertion fallback or hook replay is
introduced. Prepared allocation evidence applies only to the selected finite path.

Supported sparse output constructors use hgl-rust-direct-deltas to evaluate
scalar operands once, retain constant key aliases cold, and publish directly
into prepared child endpoints. No intermediate delta vectors are constructed.
statements additionally receives Option<&Ty> for the declared temporal result,
propagated through nested blocks. Rolling returns publish ordinary arrival V into
the typed window; ordinary helper bodies pass None. Window-specific delta_value
and all_valid queries delegate to hgl-rust-windows; metadata queries retain the
existing statically selected endpoint path.

Current Tuple node-local copies and complete results lower through
hgl-rust-snapshot-slots; ordinary aggregate constructor assembly lowers through
hgl-rust-snapshots. Owned optional positions preserve unset data independently.
No structural-delta observation or canonical type identity is substituted.

Native text arguments from iteration child endpoints borrow the typed scalar
column through the existing prepared input token. They do not clone text or
convert endpoint identity to an ordinary owning argument.

Own-output complete ordinary assignment delegates to structural_publication.
Required projected scalar and native text child reads check existing input
validity before access; an unset child never becomes a default scalar. Prepared
text projections keep read-only native borrows and independent local ownership.

Dense fixed List iteration visits unset child endpoints for metadata. Reading
its payload uses the same checked required-child transport as direct projection;
valid zero, false and floating zero remain available values. This guard changes
no iteration selection or metadata query and holds in debug and release.

May use snapshot_views for retained ordinary observation presence and List length.
Required scalar/Boolean payload reads preserve value.unset_read; let retention and
Tuple constructor copying preserve absence without consuming payloads.

Prepared publication lowering consults execution_proof owning-argument classification
for atomic scalar aggregates. These stack payloads publish directly through the
reserved atomic destination, including first metadata observations; owning and
snapshot expressions retain their dedicated transport paths. Scalar set query
emission delegates membership semantics to keyed.

Prepared output assignments reuse return publication transport without returning
from the handler; subsequent assignments and mutations retain source order.

May use scalars. Byte comparisons and lengths borrow typed scalar slices, including atomic/global/snapshot projections, instead of retaining owning copies. Prepared byte constructors use a single authorized destination and complete preflight. Ordinary fallback construction retains phase and errors. Borrowed ordinary List indexing uses the existing immutable list::global_index transport, allowing retained byte comparisons without a mutable context borrow.

Prepared publication and ordered global writes delegate to hgl-rust-prepared-hooks. Independently retained Any locals and returns delegate to hgl-rust-any-prepared. Compiled box comparison borrows typed sources, preserving fallible contained capabilities.

Generated-code formatting shares the crate's append sink across emitter modules; the helper performs the same infallible String formatting and introduces no runtime behavior.

Checked native image calls use descriptor-selected static module symbols. Legacy
input-view helpers retain their Native trait boundary; image calls need no supplied
Provider type. Interface declarations and legacy adapters share signature emission.

This module now lives in hgl-rust-codegen below executable program assembly;
its hgl-rust facade path and module ceiling remain unchanged.

May use native_image for pure selected provider module naming.

Fixed temporal REF-list Index values read only the selected opaque designation.
Index metadata delegates to observed::input_with using the checked expression
emitter, preserving left-to-right guards and fallible range checks. Returning a
selected REF neither snapshots its target nor copies collection payloads.
