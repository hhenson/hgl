# hgl-rust-finite-domains

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `finite_domains` module of `hgl-rust` (`crates/hgl-rust/src/finite_domains.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold checked source-domain analysis, separate from storage capacity planning.
`constants` traverses checked hooks and returns closed data while preserving
runtime expressions for normal hook evaluation. `include` emits accumulation of
materialized structural delta key paths, with each nested collection attached
to its actual parent. `DECLARATION` supplies the private generated key-path tree.
The output is used exclusively before startup; no name lookup or schema dispatch
is added to publication. Runtime scalar key identity still belongs to hgl-keys.

`mutations(&[Statement])` counts the maximum syntactic membership changes in one
handler traversal. Capacity planning combines source publication widths and the
finite replay horizon to reserve primitive runtime-key slots.

`widths(plan, base)` emits one per-output width in dependency order. Direct
mutation sites add, branches take maximum, and each added-element loop multiplies
its own input publication width. Independent nodes never multiply one another.
Constant integer induction loops use exact iteration counts from
`hgl-rust-mutation-bounds`; unknown mutation loop bounds select existing whole-adapter
execution via the reexported `prepared(plan)` proof decision.

Growing child paths merge by their exact constant indices without multiplying nested domains. Removed-only indices reuse the capacity established by prior admitted appends.
`direct_arrivals(plan)` reexports the source schedule proof count. Capacity planning
adds this count once across independent generators; replay list lengths remain
accounted for separately.
