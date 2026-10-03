# Card: hgl-flow-check

Pure control-flow proof analysis over checked IR. Dependency hgl-rust-ir;
budget 150 source lines. Public: terminates, facts, merge_facts, handler_facts.
Preserves endpoint-specific validity/modification and contextual presence facts
through short-circuit conditions and reachable branches. No runtime effects,
new proof rules, shape-dependent guards or structural payload inspection.
Acceptance: existing delta accessor and nullable short-circuit regressions.
