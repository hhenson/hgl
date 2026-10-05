# Card: hgl-harness

Status: accepted

Execute ordered checked test setup before and between fresh graph runs. Uses
hgl-harness-ir, hgl-value-eval, hgl-eval-data, hgl-rust-ir and hgl-source; budget 250 source
lines; no kernel or provider dependency.

execute(test: &Test, materialize: impl FnMut(&TemporalLiteral) ->
Result<Literal, EvalError>, eval: impl FnMut(usize, PreparedEval) ->
Result<CapturedEval, String>) -> Result<usize, String> owns one lexical
Evaluator and returns the existing assertion/eval inventory count.

Execute ordinary setup, mutations, assertions and evals once in written order.
Prepare all eval arguments before invoking its generated graph callback; stop
on the first error. Only executed branches materialize literal recipes. An
executed unused binding still validates. Expected values execute after eval.
The callback returns owned captures after graph teardown. Existing HGL replay
and record remain the only scheduling/capture implementations.

Tests cover ordering, skipped branches, unused invalid bindings, interleaved
constant/temporal arguments, mutation between evals and no callback after failure.

Compare captures by logical length and sparse tick positions. Work is bounded
by supplied expected cells and captured publications, never by a silent horizon.
