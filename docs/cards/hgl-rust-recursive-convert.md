# Card: hgl-rust-recursive-convert

Cold finite recursive native/checked-value conversion emission. Uses source,
rust-ir, rust-layouts and rust-checked-data; budget160. `markers(plan,decode,encode)`
emits one typed pair of conversion methods per reachable recursive batch member.
Callbacks handle nonrecursive fields through existing exact ordinary conversions;
recursive edges call the statically selected member method. Metadata collection
uses complete immutable batches, including empty/silent roots. Decoding checks
exact nominal identity and required field presence. Encoding preserves every
present descendant and complete reachable batch metadata. Owning Box allocation
is confined to these cold materialization/capture boundaries.
