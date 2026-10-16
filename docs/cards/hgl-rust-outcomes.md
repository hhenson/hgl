# Card: Rust Result/Error expressions

The outcomes emitter lowers checked constructors, tag predicates, guarded payload
and diagnostic properties, and explicit propagation. Public functions consume
checked Value IR and a callback that emits an ordinary operand; no source syntax
is checked here. `construct(&Value, impl Fn(&Value)->String)->String` and
`query(&Plan,&str,&[Value],impl Fn(&Value)->String)->String` are its surface. Typed direct projections borrow prepared selected slots; ordinary owning
helper data uses existing GlobalValue retention. Helper propagation returns Err
data; node propagation enters NodeError::from_error without replacing code or
provenance. Equality is handled by the checked collection comparison emitter, with
only the tagged branch compared.

Ordinary Result predicates enclose the complete method call for condition emission.
Authored native-local controls cover both tags, negated predicates and selected
payload reads, with one provider invocation per non-silent evaluation.

May use source_slots. No third-party dependency, source semantic fallback,
provider name lookup or per-value type dispatch. Generic owning expression paths
may allocate; typed prepared controls state their narrower zero-allocation proof.

The finite unseeded Result controls measure complete prepared publication and
retention, including nested String payloads and selected primitive projections,
with zero startup, successful-tick and cleanup allocations. Finite guarded String
payload returns now copy from their typed global slots directly into the prepared
scalar output. Ok/Err alternation, fixed fields, checked list indexes, repeated
longer/empty payloads and nullable Error metadata presence retain that same proof.
Index operands execute once; capacity failure preserves output data and metadata.
Ordinary nullable payload locals still retain their owning Option representation,
and unknown runtime diagnostic data retains its generic allocation boundary.
