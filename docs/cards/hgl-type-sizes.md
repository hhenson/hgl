# Card: hgl-type-sizes

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
