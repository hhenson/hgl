# Authored REF transport snapshot

These four HGL files are byte-for-byte copies of the paths and immutable
hgraph_std revision recorded in [PROVENANCE.json](PROVENANCE.json). Preserve
all source assertions. They cover 24 named groups and 50 evaluations.

The installed Std pin remains the compatible Native snapshot until all six REF
suites can execute. This snapshot makes the supported four suites reproducible
in ordinary cargo CI meanwhile. reference_matrix.rs uses normal source/library
loading and target-module suite planning with the entire production Std and
native/stdlib, then instruments successful evaluations with a real allocator.
Both debug and optimized tests run all 24 groups and assert 50 evaluations,
zero failures, and zero allocations in every successful tick, including first
sampling. The required-read error case retains its authored error expectation.

The other two suites contain four conditional ownership groups and are not
included: their source remains explicitly unsupported and unexecuted.
