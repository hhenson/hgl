# Card: hgl-harness-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `harness_check` module of `hgl-semantics` (`crates/hgl-semantics/src/harness_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Check ordered lexical test steps without source name resolution or expression
execution. Uses hgl-source, hgl-rust-ir, hgl-harness-ir, hgl-eval-data,
hgl-static-values and hgl-flow-check; budget 180 source lines.

Check trait supplies statement, boolean, evaluation and expected callbacks to
the frontend resolver. block(source,scope,plans,checker) checks each step in
source order, requires exact boolean guards, appends independently selected
plans and returns checked steps. All branches are checked. Branches clone
lexical bindings/provenance/facts and preserve globally fresh binding indices;
outer mutation authority remains unchanged. Duplicate names in one block fail;
shadowing outer bindings is lexical. Existing flow facts refine nullable locals.

BindEval requires a non-void graph output and binds a readonly contextual
List<Nullable<delta<output>>> checker type. Atomic delta reduction retains the
exact ordinary payload. This is not a general nullable source sequence type.
Expected values retain their existing after-run execution boundary. No provider
execution, runtime graph selection or replacement scheduling is introduced.

Raises checks its body in an ordinary lexical scope, without executing it; locals cannot escape.
