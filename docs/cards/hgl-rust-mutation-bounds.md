# hgl-rust-mutation-bounds

`mutation_width` combines source publication widths with branch and loop mutation
counts. Integer induction loops initialized from literal or retained local
constants have exact iteration bounds, including nested independent loops. The
analysis never executes hooks; other runtime while loops remain outside its proof.
