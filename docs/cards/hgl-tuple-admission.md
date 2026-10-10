# Card: hgl-tuple-admission

Own ordinary Tuple constructor and helper ABI admission. The
internal tuple_admission module has an initial 60 source-line budget within the
unchanged hgl-semantics crate limit. It uses ir, tuple_values and structural_admission.

Public surface: `value_arguments(&[Value]) -> Result<(), String>`, `expected(&Ty, bool) -> Result<Ty, Issue>`, `constructor_child(&Value, bool) -> Result<(), &'static str>` and
`result(Value, &Ty, &Ty, bool) -> Result<Value, String>`. The booleans distinguish
constant ordinary execution and node publication, respectively. expected and result re-export the structural_admission owner; expected selects the existing implicit node delta context; value functions retain
ordinary result context. Results retain
canonical type compatibility and existing payload-access checks. Implicit Tuple
outputs admit complete ordinary values; explicit delta requirements stay exact.

Runtime native collection, text or bytes children require prepared retained observation
storage before Tuple assembly. Unsupported native collection-child construction
and publication reject before emission, using ordinary uncatalogued diagnostics.
Constant ordinary construction remains admitted, including the published
independently_retained_children example. Native value-function collection mutation
is not a new allocation-proof boundary. No source identity, absent-child default,
delta error code, runtime type test or dependency is added.

Acceptance: shared runtime Tuple constructors and retained observations; published
constant example independence; unsupported native collection publication controls;
wrong ordinary element types retain ordinary diagnostics. Prepared String child
construction and native scalar helpers allocate zero times per evaluated cycle.

Retained or directly projected Tuple text arguments to ordinary value functions
reject their unprepared owning ABI; native helpers retain prepared read-only
scalar borrows. Fixed scalar helper arguments continue through their native ABI.
