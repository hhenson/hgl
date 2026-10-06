# Card: hgl-rust-recursive-convert

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `recursive_convert` module of `hgl-rust` (`crates/hgl-rust/src/recursive_convert.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Cold finite recursive native/checked-value conversion emission. Uses source,
rust-ir, rust-layouts and rust-checked-data; budget160. `markers(plan,decode,encode)`
emits one typed pair of conversion methods per reachable recursive batch member.
Callbacks handle nonrecursive fields through existing exact ordinary conversions;
recursive edges call the statically selected member method. Metadata collection
uses complete immutable batches, including empty/silent roots. Decoding checks
exact nominal identity and required field presence. Encoding preserves every
present descendant and complete reachable batch metadata. Owning Box allocation
is confined to these cold materialization/capture boundaries.
