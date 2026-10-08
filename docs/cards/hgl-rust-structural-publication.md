# Card: hgl-rust-structural-publication

Status: accepted for the bounded complete ordinary publication slice.

Own complete ordinary publication and own-output assignment emission. The module
has an initial 220 source-line cap inside the unchanged hgl-rust 8940 cap. It uses
layouts, snapshots and snapshot_slots. Layout/cold capacity and observation
capture/read remain with those existing owners.

Public surface: `native(&Ty, &str, &str) -> String`, `prepared(&Ty, &str, &str)
-> String` and `assignment(&Value, emit) -> Option<String>`. native publishes
complete supported native positional values. prepared publishes independently
retained typed slots. assignment selects this same transport without terminating
the handler. Inputs use canonical Ty; synthetic slot identities are emission-only.

The [complete publication rule](../../external/hgraph_spec/language/docs/design/structural-value-publication.md)
requires fixed Tuple/Struct/List positions to apply present children and invalidate
previously valid absent children. Map output retains exactly the source membership:
absent keys are removed, present valid members publish recursively, and present
invalid members must already exist and remain invalid. Source timestamps are not
copied. Existing sparse delta application remains with its delta owner.

Map capture establishes prepared KeyId order independently of live-member insertion
order. Publication merges that indexed sequence with the sorted prepared output
domain in linear domain-plus-source work. Every key and child destination is
prepared before start; no per-cycle allocation, lookup by name, dynamic type test,
resizing domain or provider replay occurs. Unsupported native Map/List result
storage rejects during admission; native source order is never assumed sorted.
Native positional publication is limited to prepared-safe scalar children.

Publication checks retained presence before applying any child. Empty/wholly
invalid results fail through the existing uncatalogued profile path; new invalid
Map members require existing destination membership or fail. These cases remain excluded. Exact
List reconciliation covers fixed lengths; existing Tuple growing-List retention
keeps its previous present-prefix publication without claiming shrink semantics.
No empty tick, scheduling rule, reference or required-read error code is introduced.

Acceptance: the shared four/eight publication matrix and allocation-counted own
remove/reinsert, key-registration order, nested Struct/List/Map, scalar text,
initial sparse and own-output controls. Ordinary reverse-order Map identity and
specific native collection profile rejection are separate controls.
