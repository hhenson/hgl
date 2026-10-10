# Card: hgl-structural-admission

Status: accepted for the bounded complete ordinary publication slice.

Own complete ordinary result and own-output compatibility. The internal module
has an initial 100 source-line cap inside the unchanged hgl-semantics 9590 cap.
It uses checked IR, tuple_values and endpoint_check. Tuple constructor and helper
ABI policy remains in tuple_admission.

Public surface: `collection(&Ty) -> bool`, `result(Value, &Ty, &Ty, bool) ->
Result<Value, String>`, `expected(&Ty, bool) -> Result<Ty, Issue>` and
`assignment(Value, Value, &Ty) -> Result<Statement, String>`. result distinguishes
node publication from ordinary value-function return, retains supported current
observations, and checks canonical result compatibility and payload access.
expected preserves the implicit node delta context. assignment delegates the
same boundary for an existing own output. Explicit delta requirements stay exact;
source types, categories and diagnostic catalogue identities never change.

The [complete publication contract](../../external/hgraph_spec/language/docs/design/structural-value-publication.md)
admits nonempty retained Tuple, concrete Struct, fixed List and Map values.
Fixed children retain independent validity; invalid source Map keys must already
exist in the destination. Growing List reconciliation, empty/wholly invalid
publication and creation of invalid Map members are outside this slice. The
previous Tuple growing-List retention path remains separate. Retained shapes
exclude Set, enum, family, recursive and atomic children until their prepared
ordinary transport exists. Native collection or variable-width structural result
children reject before emission; ordinary constant identity remains unchanged.
This also rejects native owning aggregate constructors and ordinary owning locals
returned or assigned to atomic and rolling outputs, including a native local
holding a rolling arrival. These values have no prepared result transport in this
backend profile. Prepared snapshots, observed locals, direct whole-value
observations and retained configuration remain separate; ordinary constant
function construction is unchanged.
Variable-width child reads require prepared ownership or the admitted native
text borrow, never an unprepared clone. No absent-child default or new error code
is supplied. Runtime excluded cases retain existing uncatalogued failures.

Acceptance: shared four groups/eight evals; complete and sparse publications
have distinct validity; removal, existing invalid membership, idle persistence,
nested fields and own-output assignment execute with zero per-cycle allocations.
Reverse-order native collection publication rejects with its specific ordinary
profile diagnostic; scalar ordinary and explicit delta matrices remain intact.
