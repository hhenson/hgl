# Type and wiring review

Status: reviewed 2026-09-27 against hgraph
[`2738ca768`](https://github.com/hhenson/hgraph/tree/2738ca7688ec13602a97dd33489f3842ab757618).
Rules are upstream contracts; the Rust gaps below are not implemented by this review.

The compiler emits Rust that wires a graph when run. The runtime therefore
owns the wiring interface, type resolution and operator selection. Descriptions
remain complete, reusable data. A checker resolving a call must reach the same
answer as runtime wiring (WIR-14), rather than develop a second interpretation.

## Rules carried forward

- Argument inference removes REF at every depth. A written REF pattern binds
  beneath it. Stated types retain references; a variable forming the whole
  requested output retains them too. Nested output variables use argument
  inference. Candidate matching considers the requested output first (WIR-6–12, 17).
- Structural projections preserve the selected field's declared type and add
  no node. Projection through REF needs a runtime node that publishes a reference
  to the selected part (WIR-5, 13).
- Bundles pair fields by name, independent of order. Names constrain matching
  only when both bundles are named, recursively and at child/service boundaries
  (WIR-15). Matching is not storage-layout equality.
- An operator declares its candidates' minimum contract. Candidates may refine
  types and add parameters, including required parameters. Calls pass extras to
  candidates; missing required extras reject that candidate. Optional contract
  parameters may be absent. Wider candidates are rejected at registration;
  selection uses each candidate's signature and reports ambiguity (WIR-16–24).
- Failure reports identify the declared call, graph path, argument types and
  candidate rejection reasons. WIR-4's caught-failure rule stands, but enforcement
  remains deferred with conditional wiring; WV-10 is an accepted runtime gap.

HGL `map<K, ref<V>>` means `TSD[K, REF[V]]`, with no outer REF. Explicit
`ref<ref<T>>` is invalid source; substitution of `T = ref<U>` into `ref<T>`
collapses to `ref<U>`. The compiler-tools experiment tests substitution,
not generic call inference. The source agreement lives in upstream
[type extensions](https://github.com/hhenson/hgraph/blob/2738ca7688ec13602a97dd33489f3842ab757618/language/docs/design/type-extensions.md).

An operator candidate and a selected native implementation part are different
contracts. Native parts still match the shared signature exactly. C++ parts and
providers belong to hgraph; Rust parts and providers belong here.

## Rust audit

| Area | Current implementation | Required change |
|---|---|---|
| Scalar compiler | `hgl-compiler` checks i64 calls, emits construction code and inlines graph composition | Retain this subset; generic, collection and operator syntax remains unsupported |
| Type vocabulary | `TsType` has concrete recursive shapes; bundles have field names but no nominal identity | Add nominal identity separately from representation; keep exact shape equality distinct from matching |
| Description validation | `hgl-plan` accepts equal ordered shapes and an outer REF source of the exact target shape | Share recursive compatibility across edges and child boundaries; admit equivalent shapes only with a working binding plan |
| Fixed bindings | `hgl-bindings::sync_fixed` pairs child slots by position | Resolve field-name mappings during construction; reuse them through whole, assembled and REF bindings, including nested graph captures |
| Projection | `hgl-plan::project` selects fixed fields/indices without a node and preserves REF leaves | Keep that path; REF traversal requires the separate reference-producing node |
| Resolution | `Catalog`/`Registry` look up concrete implementations; neither is an operator resolver | Add patterns, substitutions and candidate selection before emitting generic/operator wiring calls |
| Diagnostics | Compiler errors carry source spans; builder errors carry local labels | Preserve graph-call paths and declared identities for runtime wiring failures; no fallback that drops arguments |

WIR-15 compatibility must not replace Rust `Eq`/`Hash`: named A can match an
unnamed bundle that matches named B, while A and B do not match each other.

Changing only `check_edge` would accept shapes that the binding layer cannot
connect correctly. Sorting bundle fields would also change their positional
layout. Build a mapping once and retain it; do not add per-tick field-name scans.
The same requirements apply beneath TSD, TSL and REF and across child boundaries.

## Evidence and acceptance order

The local [Wiring](../runtime_spec/wiring.md),
[cases](../runtime_spec/cases_wiring.md) and
[validation directory](../runtime_spec/validation/wiring/README.md) are copied
unchanged from the revision above. The overview, conformance index and scalar
text rules are synchronized too. The overview's superseded statement that a
Wiring chapter is still owed is removed in favour of its settled point 2.
Scalar set/map text has no guaranteed order;
equal containers may render differently. It is not a type-identity test.

The archived assessment has 40 runtime observations matching both references,
five matching one, and four settled by owner rulings despite neither matching.
Its six HGL observations concern the **C++ compiler**, not the Rust compiler.
The report records subsequent C++ corrections separately; these historical
observations must not be relabelled as results from current main or from Rust.
No new runtime measurements are claimed here. CI regenerates the assessment
and checks it against the recorded result.

Before extending compiler coverage:

1. Validate concrete matching and binding together: reordered bundles must
   publish `a * 10 + b = 12`, named/unnamed bundles match, different named
   bundles fail, and recursive REF alternatives follow target ticks. Repeat
   through TSL/TSD nesting, assembled inputs, rebinding and child boundaries.
2. Add one runtime resolver with distinct argument, stated and requested-output
   modes. Replay WIRE-GENERIC-DEPTH/TOP, REF-PATTERN, STATED, REQUESTED and
   REPEATED; preserve source-port types while resolving consumer types.
3. Add operator registration/selection. Replay WIRE-SPECIFICITY, FAILURES and
   OPERATOR-CONTRACT, including extras, missing extras, optional omissions,
   widening, repeated constraints and registration-order independence.
4. Extend the Rust checker/emitter against those same cases. Structural
   projection and projection through REF must remain distinct. Keep nested
   graphs as library constructions using the runtime's shared wiring interface.

Expected types, selected candidates, failures and tick observations precede
implementation. Accept reasoning plus either reference, retain deviations,
and use the existing owner rulings where recorded. New cases with no agreement
return to the owner. Defaults, packs, conditions, constant kernels and conditional
wiring still have deferred details; this review does not choose their semantics.
