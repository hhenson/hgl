# hgl-rust-direct-deltas

Compile-time lowering of returned sparse constructors and retained scalar aliases.
The crate uses checked IR, layouts and typed observed transport; budget 400 lines.

`prepare(&Plan)->Plan` produces an idempotent backend plan with immutable owning
literal aliases retained in node configuration. Alias reads reuse the same typed
configuration slot. It never evaluates calls, providers or hooks, nor rewrites a
mutable local as an immutable alias. Literal keys and scalar literal child payloads
are retained cold. Ordinary frontend Local identity and source admission are unchanged.

`supported(&Value)->bool` recognizes direct returned sparse constructors with
constant/configuration scalar keys, scalar input/configuration/literal payloads,
pure numeric expressions, and recursively supported sparse children. It is used
only at the return boundary by whole-plan storage proof selection.

`publish(&Value,emit)->Option<String>` emits these constructors without transient
owning vectors. Scalar operands and typed key IDs are obtained in source order
before publication. Duplicate/overlapping keys and canonical root membership are
checked before mutation; removals precede upserts. Owning input/configuration
payloads copy directly into independently reserved destinations. Unsupported
expressions retain the existing ordinary constructor path and whole-plan proof.

Acceptance: complete generated evaluate allocation counts for repeated retained
string aliases, direct keys, nested string payloads, and removal/reinsertion;
semantic compatibility tests for plans whose bounds remain unproved.

Growing returned constructors validate their constant index ranges against destination length and prepare typed children before direct scalar or nested publication. Their sparse index, value and removal storage has no transient owning vectors.
Generator normalization retains closed literal-only yield payloads in immutable
configuration slots, including owning aggregates. It leaves the time expression
and every call/provider expression at their original execution point. Repeated
normalization is idempotent. This supplies prepared transport for finite direct
generator schedules without constructing owning values during evaluation.
