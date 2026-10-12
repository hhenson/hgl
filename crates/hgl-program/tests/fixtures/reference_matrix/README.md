# Authored REF snapshots

These six files are byte-for-byte copies of the immutable hgraph_std revision
and paths in [PROVENANCE.json](PROVENANCE.json). Preserve all assertions.
They cover 28 named groups and 58 evaluations.

reference_matrix.rs runs the four transport suites (24 groups / 50 evaluations).
reference_conditional.rs runs the two ownership suites (4 groups / 8 evaluations).
Both use normal source loading, target-module suite planning and the complete
production Std plus native/stdlib. Real allocators measure every successful
cycle from first activation in both profiles. The ownership runner also measures
startup and cleanup. Required-read failures keep their authored expectations.

These immutable snapshots preserve focused coverage independently of the
installed Std revision. The installed Std pin is the same immutable revision. Its complete loaded
corpus passes in both profiles: 360 named Std tests / 957 evaluations, including
test blocks in production modules. The existing contextual fixtures add
12 tests / 22 evaluations; the combined checked corpus is 372 / 979. Separate
existing harness regression and expected-failure images also pass. Full CI
and platform validation remain separate gates.
