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

The finite unseeded Result controls measure complete prepared publication and
retention, including nested String payloads and selected primitive projections,
with zero startup, successful-tick and cleanup allocations. Returning a String
payload from a Result local still lowers through an owning `ValueSlot<String>::read`
before scalar publication. A direct `let outcome=value; if is_ok(outcome) {return
outcome.value}` probe measured one allocation on its first nonempty String
publication tick; that projection is outside the complete prepared publication proof.
