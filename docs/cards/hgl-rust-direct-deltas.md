# hgl-rust-direct-deltas

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `direct_deltas` module of `hgl-rust` (`crates/hgl-rust/src/direct_deltas.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Compile-time lowering of sparse output constructors and retained scalar aliases.
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
owning vectors for returns and output assignments. The emitted body leaves control
flow to its caller, so assignment preserves subsequent statements. Scalar operands and typed key IDs are obtained in source order
before publication. Duplicate/overlapping keys and canonical root membership are
checked before mutation; removals precede upserts. Owning input/configuration
payloads copy directly into independently reserved destinations. Unsupported
expressions retain the existing ordinary constructor path and whole-plan proof.

Acceptance: complete generated evaluate allocation counts for repeated retained
string and byte aliases, direct keys, nested owning payloads, and removal/reinsertion;
semantic compatibility tests for plans whose bounds remain unproved.

Growing output constructors validate their constant index ranges against destination length and prepare typed children before direct scalar or nested publication. Their sparse index, value and removal storage has no transient owning vectors.
Generator normalization retains closed literal-only yield payloads in immutable
configuration slots, including owning aggregates. It leaves the time expression
and every call/provider expression at their original execution point. Repeated
normalization is idempotent. This supplies prepared transport for finite direct
generator schedules without constructing owning values during evaluation.
