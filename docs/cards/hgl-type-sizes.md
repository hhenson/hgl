# Card: hgl-type-sizes

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `type_sizes` module of `hgl-semantics` (`crates/hgl-semantics/src/type_sizes.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Normalize constant list-size expressions before canonical type formation.
Dependencies source, rust-ir, value-eval, value-check; budget 180 source lines.
Public: normalize(name,evaluate), literal(expression). Normalize preserves type
names and nested applications, evaluating only explicit list-size positions.
Sizes must evaluate to nonnegative i64; direct unbounded remains the separate
sentinel. The caller supplies declaration/use-site constant context. literal
uses the same ordinary checker/evaluator for closed scalar expressions, with
no alternate arithmetic policy. Function calls and configuration names need
the compiler's normal resolution context, not string substitution.
Acceptance: nested type equivalence, arithmetic precedence, configuration
sizes, rejection of runtime/wrong-type/negative/overflow sizes.

Rolling maximum/minimum expressions use the same checked lexical constant
callback, then hgl-window-types validates kind and range. Omitted minimum equals
maximum. Canonical microseconds preserve equivalent duration spelling identity;
tick counts never implicitly convert to duration bounds.

expressions(name) collects declaration-owned bound syntax recursively without
calling providers, validating sizes or manufacturing placeholder values. List
bounds and rolling Max/Min are scalar expression positions; only their payload
arguments are traversed as types. Normalization later validates actual literals.

normalize_checked is the structured form of normalize; it returns Issue with
rolling.size_kind or rolling.size_bounds at the offending size argument range
relative to the input type spelling. Normal normalize delegates to it, keeping
ordinary and rejection-check rules identical. Nested ranges are translated to
the containing annotation; declaration token ranges recover physical locations.
