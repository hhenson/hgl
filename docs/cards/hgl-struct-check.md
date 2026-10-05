# Card: hgl-struct-check

Check ordinary finite-field constructors and infer their type parameters.
Uses hgl-source, hgl-library, hgl-value-types, hgl-value-access and hgl-rust-ir;
budget 450 source lines. No runtime or third-party dependency.

Public surface: `Constructor::{new,next,checked,finish}`. A session accepts
library/module, constructor name, source fields, optional expected type and
ambient function type bindings.
The frontend supplies each checked field value in response to the next request.
Explicit applications supply all arguments. Inferred constructors unify nested
field evidence with expected nominal arguments, reject conflicts/unresolved
parameters, then validate the specialization and field types. Every supplied
field must be named, unique and declared; all fields without defaults must be
present.
Checked Construct IR preserves written argument order and declared field indices;
omitted non-null fixed scalar defaults follow supplied arguments in declaration
order. Checking never executes supplied field values; defaults are already
constant literals and are retained only at construction. Delta construction
uses a separate checker and never fills omitted fields from defaults.
Context-dependent fields are checked only once their concrete expected types
are available.
In particular, `get(global_state, ...)` supplies no independent type evidence;
concrete sibling fields may supply its expected type without changing runtime
argument order.

Acceptance: explicit/inferred/contextual and nested constructors, constraints,
written field order, repeated argument conflicts, and read-only borrowing/copies.

`schema_sizes` discovers declaration-owned size expressions in reachable nominal
fields for evaluation by the normal source checker before schema specialization.
Dependencies also include hgl-type-sizes; recursive references are visited once.

Omitted schema defaults may contain ParsedLiteral::Contextual. Emit the typed
TemporalLiteral recipe after supplied fields, in declaration order, for ordered
construction. Existing fixed literal defaults and sparse-delta omission rules
remain unchanged. Contexts without provider construction reject recipe use.

Resolve retained enum member defaults with their declaration module through
hgl-enums before constructing an omitted ordinary field. May use hgl-enums.

Optional field positions come from declaration metadata. Reject explicit null
for required fields before evaluating any constructor argument. Omitted or
explicitly null optional fields contribute no payload to Kind::Construct;
present fields retain their normal exact type and written evaluation order.
An empty Construct for an all-optional schema is a present complete value.

Abstract constructors are rejected. Inherited fields retain ancestor-first order,
source declaration scopes, introducing optionality/defaults and exact generic
substitutions. Expected family membership can supply an exact concrete member's
generic arguments before field checking. Widening itself belongs to family-values.

Unresolved inherited/direct generic field inference first recovers the canonical
source argument schema. Ordinary field payloads remain projected; inferred and
explicit nominal specializations retain identical nested atomic boundaries.
schema_sizes traverses declared ancestors as well as own fields, retaining each
ancestor module for checked constant-size expressions. Imported inherited list
bounds therefore use the introducing declaration's helper scope.
