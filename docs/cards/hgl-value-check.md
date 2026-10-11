# Card: hgl-value-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_check` module of `hgl-semantics` (`crates/hgl-semantics/src/value_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Ordinary value authority, borrow provenance and lexical entry-effect checking
for checked HGL hook IR. Uses `hgl-source`, `hgl-rust-ir` and `hgl-library`; budget 400
source lines. No runtime or third-party dependencies.

Public surface: `ordinary`, `writable`, `field`, `provenance`, `binding`,
`helper_argument`, `validate`, `list_literal`, `indexed`, `list_operation`. The frontend resolves names, types and const
keys first, then uses these checks before emitting a plan.

`binding` preserves aggregate get provenance and enforces alias permissions;
`provenance` applies only to aggregate views, so primitive extraction is owned.
`validate` checks each lifecycle hook independently, retaining declared borrow
lifetimes through the lexical block even when a name is shadowed. Conflicts
use prepared entry indices after equal configured strings have been unified.
Constructors and global set retain their values; ordinary helpers and output
cannot retain live aggregate view authority. No runtime registry is introduced.

Acceptance: aggregate global source checking and executed fixtures in
`hgl-program`, including disjoint scopes, const-key aliases, recursive
readonly authority, exclusive aliases and explicit retention boundaries.

Ordinary list literal checking requires homogeneous constant elements and exact
contextual fixedness; empty literals need concrete context. Indexed aggregate
projections inherit root authority/provenance. Primitive projection and length
results are owning scalar observations. Push checks unboundedness, writable
receiver authority and exact item type, then supplies an explicit retaining
operation to the backend. Indexed replacement is outside this contract.

Uncontextualized nonempty ordinary literals are explicitly unsupported: the
pinned specification describes constant homogeneous list literals but does not
uniquely define their inferred fixedness. Contextual nonempty literals retain
their expected exact list type. Harness sequence typing is unchanged.

`prepare_node` initializes a node and converts supplied temporal values to input bindings and
retains ordinary aggregate const configurations in node metadata. Generator body
validation traverses while scopes and ordered timed-yield operands. Backend-only
GeneratorLocal values are never produced by source checking.

`binary_type` defines the admitted scalar binary operand/result table after
numeric widening. Checked time arithmetic admits datetime plus/minus duration,
duration plus datetime, datetime minus datetime, and duration plus/minus duration.
Duration negation is also checked. Scaling, division and date arithmetic remain
outside this implementation slice; no wrapping behavior is introduced.

Access classification is delegated/re-exported from hgl-value-access, including
`observed`. Typed Delta globals retain lexical aggregate borrowing. Ordinary
constructor/list/global/owning assignment and matching delta publication are
explicit retention boundaries; helper returns/calls cannot escape observations.
Ordered delta children are traversed for entry effects without weakening borrow
conflict checks. Delta binary comparison and field/index inspection are rejected.

`aggregate_checked<E>(expr: &Expr, expected: Option<&Ty>, constant_context: bool,
check: impl FnMut(&Expr, Option<&Ty>) -> Result<Value, E>) ->
Result<Value, E>` centralizes
ordinary list and positional tuple checking. The caller supplies its existing
expression checker; element admission, constant-expression requirements and
exact contextual list fixedness are unchanged. Tuple arity and each position's
expected type are checked independently, retaining source evaluation order.

A constant context may retain local expression IR until source-order ordinary
evaluation closes it. This does not admit nonconstant runtime list literals.

Re-export `hgl-value-constant::context_free(&Value) -> bool`; this purely classifies closed evaluator-supported
ordinary expressions, including lexical arguments and locals inside ValueCall
blocks. It rejects external locals, contextual recipes, prepared values,
providers, native/capability/global/endpoint effects and unsupported or unbounded
control flow without executing anything. The frontend may then use the ordinary
evaluator for checking-time diagnostics; short-circuit and branch execution
remain the evaluator's responsibility. Contextual assertions remain deferred.

Delta traversal includes typed Add/Remove keys and both Keyed map operands;
retained keys obey the same constant/provenance checks as child expressions.

Binary ==/!= accepts exact ordinary set/map types. Their unordered recursive comparison is separate from sparse delta operations.

aggregate_checked preserves typed source Issue failures through contextual
child-check callbacks. String callers use the same generic entry point.

Runtime Tuple children delegate prepared ownership admission to tuple_admission.

Ordinary len admits bytes and returns its octet count. Bytes equality and unsigned lexicographic ordering are admitted; indexing, mutation and encoding remain excluded. Existing runtime list-literal admission is unchanged.
