# Card: hgl-rust-generators

Lower checked generator source bodies into typed Rust resume machines under
ADR0015 and its pinned operand-order clarification. Uses local `hgl-source`,
`hgl-rust-ir` and `hgl-rust-values`; budget 500 source lines. No third-party or
runtime execution dependencies. Source admission belongs to the frontend.

Public surface: `Generator::lower(body)`, `Generator::{fields, initialize,
start, evaluation}` return emitted storage, construction, restart and evaluation
fragments for a checked scalar-output generator. The generator stores a resume
position, one independently owned pending scalar, and optional typed lexical
locals. Structured if/while bodies lower to numbered blocks with explicit
successors; completing a block advances once, a future yield resumes at its
successor. Helper-call locals remain lexical and cannot collide with hoisted
source locals. Node restart clears all generator storage and requests its first
evaluation; it never runs the body in start.

Each reached yield evaluates its time operand, then its payload exactly once.
Signed checked microsecond addition resolves duration targets after both
operands, admitting representable past targets before the engine epoch. Past
targets skip publication, due targets publish then continue, and a second due
publication fails after operand evaluation. Future targets park an independently
owned payload before suspension. The existing source alarm checks scheduling
bounds; resumed publication moves that payload without reevaluating operands.
Local ownership uses the existing ordinary retention contract and prepared typed
representations. No runtime type dispatch, names, borrowing registry or shared
ownership enters the resume path.

Acceptance: source-generated debug/release execution for all eight scalar
outputs, nested loops/conditionals, hoisted shadowed locals, past/due/future
ordering, failures and duplicate timestamps, operand once-only effects, pending
payload independence, source completion and restart reset. Root integration
checks use only pinned spec cases and HGL implementation.
