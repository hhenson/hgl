# Card: hgl-rust-snapshot-views

Own emitted views of retained ordinary values and their unset state. The module
has an initial 160 source-line cap inside the unchanged hgl-rust 8940 limit.
It uses layouts and snapshots; source canonical types and checked IR remain
unchanged. A view is an existing typed prepared ValueSlot and a stack Boolean.
No runtime type, capacity, dependency or source category is added.

Public surface: `view` projects an observation without consuming its payload;
`payload` checks presence before a required read; `copy` independently retains
presence and copies prepared slots only when present; `constructor` copies an
observation into a prepared optional child; `length` consumes List presence and
reads the existing prepared length; `publication` selects the existing scalar
read or structural publication boundary. All accept checked Values and a cold
emission callback. The returned Strings are generated Rust, never runtime data.

Copying an unset child does not read stale slots or supply a default. Its
reserved destination remains available for later valid evaluations. Constructor
copies preserve absence recursively. Required scalar and List-length reads fail
through NodeError::coded with execution code value.unset_read. Direct temporal
root validity failures, bounds failures, missing keys and the existing excluded
whole structural publication profile retain their distinct identities. No new
empty or wholly invalid publication behavior is introduced.

Finite slot preparation and native scalar borrowing stay with snapshot_slots;
private layouts and cold recursive bounds stay with snapshots. Source type and
ownership admission stay with the semantic owners. Per-cycle operations use
only typed slots, presence and existing prepared columns: no allocation, name
lookup, type test or dynamic capacity is introduced.

Acceptance: Spec79 required Number/Flag/List reads match exact execution codes;
present zero/false controls pass; copying partial Tuple children and assembling
a Tuple preserve absence and independent ownership. Shared error tests run through lifecycle-faithful simulation and match the code.
Successful first/repeated zero/false/partial retention evaluations are counted
separately and run with zero allocations. Error construction can allocate; the
measured harness reports failed-cycle allocation counts separately, always stops
the graph, and preserves the primary error plus cleanup failures.

List indexing still requires the parent payload and existing bounds checks before
retaining the selected child; this extension supplies no absent-List indexing rule.

May use observed. byte_slice(&Value, Option<(&str,String)>, &str)->String borrows bytes from selected typed storage or a live scalar endpoint, preserving required child validity and avoiding owning copies.
