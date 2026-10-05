# hgl-rust-mutation-bounds

`mutation_width` combines source publication widths with branch and loop mutation
counts. Integer induction loops initialized from literal or retained local
constants have exact iteration bounds, including nested independent loops. The
analysis never executes hooks. The result is absent for unknown mutation loop
bounds. `mutations` exposes the syntactic effect count used to recognize relevant
loops; it is not itself an iteration bound.
