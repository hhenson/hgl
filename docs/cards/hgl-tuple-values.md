# Card: hgl-tuple-values

Classify ordinary Tuple observation ownership and positional projection in the
frontend. The `tuple_values` module has an initial 100 source-line budget within
the unchanged hgl-semantics limit. It uses checked IR and endpoint payload checks.

Public surface: `endpoint`, `supported`, `retain`, `indexed`,
`delta_result`, `nested_snapshot`. Canonical ordinary types remain exact. `retain` marks a supported
current Tuple read (or a text element explicitly selected by the constructor caller) for independent prepared ownership. `indexed` resolves fixed
Tuple positions, preserving snapshot representation and source endpoint identity,
and checks ordinary List indexing. Signals retain their existing payload ban.
Implicit ordinary Tuple results may construct complete publications; explicitly
required delta<T> results retain the catalogue's exact compatibility requirement.

Runtime Tuple construction follows ordinary-tuple-values.md; nonempty ordinary
List literal restrictions remain. Complete tuple equality uses recursive ordinary
data equality. Unsupported snapshot interop is diagnosed at its source boundary,
without new delta codes or absent-read defaults.

Acceptance: shared retained Tuple tests, native runtime constructors, constant
phase rejection controls, ordinary wrong-element diagnostics and existing delta
rejection suites.
