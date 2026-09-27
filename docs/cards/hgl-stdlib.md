# Card: hgl-stdlib

Status: first fresh-run port; [contract](https://github.com/hhenson/hgraph_spec/blob/main/language/docs/design/node-authoring.md).

Hand-written lowering specimens from upstream HGL. May use `hgl-native`,
`hgl-describe`, `hgl-kernel`, `hgl-store`, `hgl-types`. Budget: 220 lines.
Tests may use `hgl-testkit` and `hgl-alloc-count`.

Public surface: `BitAndI64`, `SampleI64`, `DedupI64`, each implementing `Node`
and `Buildable`; `register_all(&mut Registry) -> Result<(), BuildError>`.
Registration names are `hgraph.operators.bit_and.i64`, `hgraph.std.sample.i64`,
`hgraph.std.dedup.i64`: concrete implementation keys, not new HGL identities.

Use the existing plain node recipe. NAT-4: bit_and listens to both inputs and
requires both valid; sample listens only to signal and requires both valid;
dedup listens to ts, emits its first value and then only changes. Equal sample
and bit_and outputs still tick. NAT-5: dedup history is independent per instance,
retained over idle cycles, and not checkpointed in this fresh-run specimen.

Done: every reasoned case passes through real replay/record graphs; metadata
checks verify passive sampling; two graph instances have independent state.
No per-tick allocation. Compiler generation and recovery are not implied.
Mutants, on a copy: dedup starts with Some(0); sample admits signal without ts;
sample marks ts active (caught by admission metadata, even if its guard masks
extra output). Each must fail a named test.
