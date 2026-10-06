# Card: hgl-flow-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `flow_check` module of `hgl-semantics` (`crates/hgl-semantics/src/flow_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Pure control-flow proof analysis over checked IR. Dependency hgl-rust-ir;
budget 150 source lines. Public: terminates, facts, merge_facts, handler_facts.
Preserves endpoint-specific validity/modification and contextual presence facts
through short-circuit conditions and reachable branches. No runtime effects,
new proof rules, shape-dependent guards or structural payload inspection.
Acceptance: existing delta accessor and nullable short-circuit regressions.

node_guard applies implicit readiness to the admitted publication inputs before source handler checking.
