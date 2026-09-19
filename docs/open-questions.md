# Open questions

A running list of questions that do not yet belong to an exploration. Cheap to
add to: one line for the question, a line or two of context if it needs it.

When a question gets picked up, move it into an exploration and replace the
entry here with a link. When it gets answered outright, note the answer and
the date rather than deleting it.

## Unanswered

- Picked up in [0005](explorations/0005-code-shape.md#positions-to-prove) and
  [0006](explorations/0006-build-sequence.md): scripted execution of runtime
  bodies, port-or-rewrite for the front end, the endpoint-graph ownership
  model, line budgets, where the operator resolver lives, the `.hgspec` draft
  as a starting point, and the conformance corpus coming first.
- Can hgraph record the candidate chosen by every operator resolution? The
  resolver's oracle in [0006](explorations/0006-build-sequence.md) (step A5)
  depends on it. Tick recording from the reference `hgl` is no longer on the
  critical path: the runtime is checked against hgraph's `eval_node` tests,
  and the `.hgl` corpus carries its own expectations.
- The "graduating an idea" flow in [README](README.md) was written for
  language ideas. Does anything found here about *implementation* graduate
  upstream, and in what form?

## Answered

- *Which language and tools for the new compiler back end and runtime?* Rust,
  end to end — [decision 0001](decisions/0001-rust-end-to-end.md), 2026-09-19.
- *Does the root README still describe this repository?* No; it was rewritten
  on 2026-09-19 when the first code (the workspace and `xtask`) landed.
