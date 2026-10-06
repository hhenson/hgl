# hgl-rust-mutation-bounds

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `mutation_bounds` module of `hgl-rust` (`crates/hgl-rust/src/mutation_bounds.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

`mutation_width` combines source publication widths with branch and loop mutation
counts. Integer induction loops initialized from literal or retained local
constants have exact iteration bounds, including nested independent loops. The
analysis never executes hooks. The result is absent for unknown mutation loop
bounds. `mutations` exposes the syntactic effect count used to recognize relevant
loops; it is not itself an iteration bound.
