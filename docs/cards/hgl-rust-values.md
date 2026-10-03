# Card: hgl-rust-values

Emit checked hook expressions, statements and nominal global value layouts.
Uses `hgl-source` and `hgl-rust-ir`; budget 800 source lines. No runtime
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
