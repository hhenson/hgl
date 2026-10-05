# Card: hgl-value-types

Resolve finite ordinary schemas against source declarations and imports. Uses
hgl-source, hgl-library, hgl-value-check, hgl-value-access and hgl-struct-names;
budget 350 source lines. Identity and declaration lookup are re-exported from
hgl-struct-names; export closure validation delegates to the same owner.

Public surface: identity, declaration, resolve, resolve_ordinary, specialize, substitute, unify. Nominal required-field structs
preserve qualified identity; ordinary lists preserve recursive element type and
exact fixedness. Visibility and recursive-schema checks remain source checks.
Type-generic required-field schemas retain the complete invariant specialization
identity. Non-null fixed scalar field defaults are checked after substitution and do not
change nominal identity or sparse publication shape. Unsupported schemas fail before publication formation.

Acceptance: existing imported/nominal struct checks and ordinary nested
struct/list construction and global configuration fixtures in hgl-program.

Specialization substitutes type parameters recursively through nested lists and
nominal applications, resolving declaration-owned names in their declaring
module and application argument names at the use site. Constraints are checked
before producing a type. Export closure follows declaration-owned field/argument references, including
phantom arguments written by the exporting declaration. Applying an exported
generic family to a private local argument does not export that argument. `unify` recursively matches source field patterns
against checked types without covariance or changing list fixedness.

Publication type resolution also handles positional tuples, maps, sets and
contextual delta<T>. Nominal temporal schemas preserve the same declaring
identity and argument invariance as ordinary schemas. Generic unification
inverts delta<T>: structural delta origins and scalar reduced forms bind the
exact originating T, including nested TimedValue/list patterns. Formation
rejects unsupported concrete origins after substitution; unresolved generic
patterns remain source spellings until a concrete specialization is selected.

Resolution and inference consume Library's checked constant sizes. This keeps
helper calls in size positions on the same source/effect/type path as ordinary
constant evaluation, without alias-dependent size identity.

Generic occurrence validation delegates to hgl-shape-obligations (an allowed
dependency), preserving the same checked argument identities. Scalar atomic
spelling normalizes before ordinary-value requirements; composite atomic
arguments remain shapes and only pass occurrences that admit them.

Resolve declared enum types before ordinary structs through hgl-enums; no generic
arguments or implicit integer conversion apply. Resolve each retained scalar
default in its declaration module before checking its exact field type.
May use hgl-enums.

resolve_ordinary resolves a complete source type with a fresh recursion scope,
then projects its ordinary payload for argument hints.

Delegates finite recursive declaration batches to hgl-recursive-types before
ordinary acyclic expansion. Exact canonical edge/root identities unify without
schema expansion. Generic arguments preserve source boundaries and all invariant
arguments. Ordinary projection and constructor inference retain complete batches.
Optional fields and the finite recursive atomic profile are admitted; structural
recursive roots and const-generic declarations remain outside this profile.
