# hgl-growing-range

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `growing_range` module of `hgl-types` (`crates/hgl-types/src/growing_range.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Shared canonical dense-index rules for cold replay preflight and typed runtime
publication. No dependencies, storage, allocation or source-expression evaluation;
budget 100 source lines.

`validate(length,items,removed)->Result<(),&'static str>` accepts cloneable i64
iterators in arbitrary written order. It rejects negative/repeated positions,
append gaps, out-of-range/non-tail removal and removed/updated overlap. With
removals, indices must be exactly a complete current tail and updates precede the
cut. Without removals, new indices form the next contiguous suffix. Empty-event
and recursive child admission remain the caller's responsibility.

Acceptance: unordered dense append, shrink-to-empty and regrowth; malformed
range negatives; complete generated growing replay/pass/record runs and runtime
empty-root validity with zero-allocation reappend.

`validate_distinct` shares the state-dependent range check, using linear scans
when source checking or membership tables have already established unique,
nonnegative indices. Prepared slot transport and observation forwarding use this
entry point; generic ordinary application and cold trace admission retain full
validation. No scratch allocation or per-tick duplicate pair scanning is needed.
