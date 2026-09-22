# Recursive graph descriptions

Status: validated and implemented for the five boundary scenarios.

The graph description remains plain, reusable data (GRF-1–3). Recursive port
shapes and fixed endpoint paths must survive ranking and instantiation.
Wiring and loaded descriptions apply the same checks before creating storage:
paths exist, types match, producers precede consumers, and whole/descendant
bindings do not overlap (GRF-4, 6–9). Separate sibling bindings are permitted.
Dynamic dictionary keys are selected during evaluation, not fixed edge paths.

A child boundary preserves the owner's binding recursively. A peered subtree
uses the same output; an assembled subtree preserves its child designations,
including empty children and later changes to captured REF designations. It adds no collection node or copied output. New
child inputs sample valid targets at the current cycle; unchanged designations
retain their observation state (GRF-10, TS-14, TS-25).

Five eleven-cycle scenarios cover a named bundle of fixed lists: owned,
assembled and mixed, plus owned and mixed parent-capture rebinding. A switch contains a keyed map; each child follows a REF,
accumulates the selected first leaf, and publishes a bundle. Updating A,
switching to B, updating detached A, replacing a timer, removing key 0 and
recreating it exercise binding and lifetime together. Observations include
every input/output level, membership, removed values and lifecycle events.
The expected output leaf is `2, 5, 15, idle, -15, remove, idle, 5, idle, -5, idle`.

The [literal expectations](runtime_spec/validation/descriptions/reasoned.json)
precede reference execution. Existing rulings remain authoritative; missing
reference observations remain missing. The [comparison report](runtime_spec/validation/descriptions/README.md) records
9,330 unanimous observations, 614 supported by one runtime, and 46 governed
by existing explicit rulings. Initial expectations and missing evidence remain visible.

The immediate deliverable is runtime and graph-description infrastructure,
exercised by Rust written in the form a future compiler could emit. Once
that target is sufficient, review the Rust compiler architecture and available
Rust libraries and tools. Then implement compile, check and emit. Decision 0001
stands: the compiler is rebuilt in Rust; the C++ compiler remains a reference.

`hgl-plan` owns the reusable descriptions, hand builder and checks;
`hgl-describe` constructs scoped instances. Their tests compare all 9,990
accepted assertions, reject malformed descriptions before allocation, and
repeat failed child startup 200 times without growing storage or subscriptions.
No third-party dependency or existing budget changed.

Boundary paths currently reject descent through a live REF capture. Pass the
whole target and project inside the child; a one-time target copy would lose
rebinding. Ordinary fixed paths and whole-target live captures are supported.
The compiler review must account for this lowering constraint before widening
that interface. Compiler parsing, checking and emission remain unimplemented.
