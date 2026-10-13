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

These immutable snapshots preserve coverage independently of the installed
Std pin. Current Std `d82bcc3` adds the separate fixed-list REF routing suite:
five groups/seven evaluations, with matching authored provenance. The complete
loaded Std corpus contains 365 named tests/964 evaluations; contextual fixtures
add 12 tests/22 evaluations (377/986 combined). Full CI and platform validation
remain separate gates.
