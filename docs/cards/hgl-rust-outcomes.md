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

May use source_slots. No third-party dependency, source semantic fallback,
provider name lookup or per-value type dispatch. Generic owning expression paths
may allocate; typed prepared controls state their narrower zero-allocation proof.
