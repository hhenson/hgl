# Card: Standalone cold wiring

The semantics `wiring_values` module retains checked provider-dependent ordinary
construction in `Plan::wiring`, in source execution order. Its `value` and
`statement` functions own wiring evaluation and lexical writes. Closed operations
continue through the ordinary evaluator. Native recipes are inert during checking;
provider-dependent conditions cannot select graph topology. Required type-size
constants still use the context-free evaluator. Helper bodies retain independent
local scopes, while captured wiring operands retain their exact binding versions.

`Evaluator::bind_checked` installs an already checked value without evaluating it.
It is used for symbolic wiring bindings and for executed recipe results. It grants
only the supplied lexical write authority. `bind_failed` remains the unavailable
result facade. Evaluator and Model cannot discover or load providers.

The hgl-rust `cold_wiring` module emits standalone recipe execution through the
existing checked evaluator and statically linked NativeContext. Registration does
not execute helpers. Each generated main executes its recipes once, including
unused initializers; it then creates the graph's data-only configuration carriers.
Cloning or instantiating a description does not execute semantic helpers again.
The ordinary standalone allocation profile is unchanged.

Native scalar carriers use private Bytes entries framed with exact canonical type,
provider identity and descriptor fingerprint. This is backend owning-copy transport,
not an HGL bytes conversion or a Serialization capability. Node construction checks
the frame and runs the typed provider's fallible ownership preflight before retaining
an independent owner. Builtin scalar results use the existing Scalar carriers.
Provider codes remain in cold construction diagnostics. This scalar checkpoint does
not yet implement Native-bearing aggregate or Any configuration transport.

Controls cover retained aliases and replacement, nested helpers, unused empty
results, independent main invocations, reusable cloned descriptions, corrupt carrier
identity, throwing helpers and inert registration. Fresh-result preflight separately
checks precise provider copy failure and once-only helper execution.

Initial module ceilings: semantics wiring_values 160 and Rust cold_wiring 160;
existing aggregate ceilings and dependency walls remain unchanged.

Unavailable ordinary wiring locals retain their checked type for later diagnostics.
A deterministic operation failure following an already deferred helper becomes an
ordered cold failure recipe, so earlier helpers execute and later helpers do not.
Existing constant checking and pre-provider construction-failure behavior remains
unchanged. A control checks failing list access, its retained failed alias, and the
exact helper count before the failure.

Native phase admission follows the checked invocation context. Ordinary composition
blocks and their nested const helpers use cold, including ordinary conditional
bodies; lexical test setup retains setup. Node checking temporarily enters the
node's lifecycle context, including lifted const functions, and restores the
surrounding wiring context afterwards. Merely using the ordinary statement checker
must not relabel cold invocations as node evaluation.
