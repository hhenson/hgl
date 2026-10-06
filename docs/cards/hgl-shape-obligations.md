# Card: hgl-shape-obligations

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `shape_obligations` module of `hgl-semantics` (`crates/hgl-semantics/src/shape_obligations.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Declaration lookup uses hgl-struct-names, an allowed dependency, so generic
occurrence requirements and type resolution select the same declaration.

Derive generic struct parameter requirements from the source declaration.
Uses hgl-source, hgl-library and hgl-value-access; budget 150 source lines. No runtime or
third-party dependency.

`validate(library: &hgl_library::Library, declaration: &hgl_library::Decl,
arguments: &[hgl_source::Ty]) -> Result<(), String>` validates canonical checked
arguments using the existing ordinary-value admission rule.
Ordinary field positions require value types; occurrences under delta permit
admitted temporal shapes. Forward requirements through generic fields and
parents, intersect repeated uses, and retain value_type for unused parameters.
Normalize scalar atomic arguments before checking occurrences. Do not weaken
existing requires constraints or admit unresolved/partial specializations.

This implements generic-struct-shape-arguments and atomic-scalar-equivalence.
Acceptance: TimedValue with atomic/structural origins; forwarded and repeated
requirements; ordinary-value rejection for composite atomic arguments; scalar
normalization before ordinary occurrence checks. A caller cannot gain a type
argument domain merely because its derived payload matches another type.

Recursive generic forwarding computes a monotone fixed point over the finite
declaration graph. Active backedges use accumulated occurrence requirements;
unchanged generic permutations therefore propagate obligations across the complete
cycle. Only after convergence do unused root parameters acquire value_type.
This preserves ordinary/delta restrictions without rejecting finite recursion.

A rolling structural field is an admitted temporal child; its payload parameter
still requires an ordinary value type. Max/Min are bound expressions, excluded
from generic type occurrences. This does not make a rolling endpoint or a struct
containing one an ordinary atomic payload.
