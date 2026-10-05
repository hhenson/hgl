# Card: hgl-rust-keyed

Typed set/map code generation and finite cold key-domain preparation. Uses
hgl-source, hgl-rust-ir, hgl-rust-layouts and hgl-rust-checked-data; budget 450
source lines. Emits static Key calls, never type-erased per-tick key operations.

`preparation(&Plan) -> String` emits cold prepared-argument traversal and static
key seeding before graph construction. `observation(&Ty, Option<&str>) -> String`
and `application(&Ty, Option<(&str,&str)>) -> String` emit exact owning sparse
key observations and membership updates; child operations are supplied by the
structural delta owner. Typed domains must already contain every key. Unknown
keys fail; the generated path does not expand a domain during execution.

Harness captures retain ordinary K, including enum identity and zone spelling.
Collection comparison continues using ordinary key equality and recursive
child equality, independent of member/entry order.

allocation(key:&Ty, shape:&str, child:&str) -> String prebuilds an inactive
collection for its prepared finite key domain using the supplied typed child
factory. Input projections are prepared after graph wiring and before startup.

`node_preparation(node, emit)` seeds literal key domains during ordinary generated
node construction, preserving callers that instantiate outside the eval harness.

May use hgl-static-values to follow immutable constant lexical origins during
cold key-domain discovery. Prepare known keys, never emit an initializer replay
or a provider call. Runtime constructor operands retain their original locals.

constructors(plan, root, width, emit) emits known constructor topology paths and
per-field widths from checked sparse source data, including immutable key
aliases. root maps a retained exact Delta type to Some(topology expression), or None
when that constructor has no retained destination in this plan; width maps
that type, storage field and constructor length to cold capacity accumulation.
emit serializes a known complete key constant. Already constructed node configuration
keys are read cold, without provider or initializer replay. Payload expressions
are traversed only for their checked shape/key metadata and never evaluated.

Known key preparation consumes retained complete Values from StaticValues rather
than reconstructing scalar literals. This preserves the full tuple/struct key
schema and written-order contextual recipes for the separate cold materializer.
Constructor topology treats growing removed indices as the third sparse storage field, preserving written operands and parent-specific child paths.

Cold prepared-value traversal decodes exact admitted composite key schemas before
calling their statically selected Key implementation. Every provider recipe has
already been materialized by ordered harness preparation; decoding cannot replay
it. Set key types are ordinary values, not child temporal publication shapes.
