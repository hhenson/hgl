# Card: hgl-value-types

Resolve finite ordinary schemas against source declarations and imports. Uses
hgl-source, hgl-library and hgl-value-check; budget 350 source lines.

Public surface: identity, declaration, resolve, specialize, substitute, unify. Nominal required-field structs
preserve qualified identity; ordinary lists preserve recursive element type and
exact fixedness. Visibility and recursive-schema checks remain source checks.
Type-generic required-field schemas retain the complete invariant specialization
identity. Optional/default, recursive and const-generic schemas are not admitted.

Acceptance: existing imported/nominal struct checks and ordinary nested
struct/list construction and global configuration fixtures in hgl-program.

Specialization substitutes type parameters recursively through nested lists and
nominal applications, resolving declaration-owned names in their declaring
module and application argument names at the use site. Constraints are checked
before producing a type. Imports and exports include every nominal argument,
including phantom parameters. `unify` recursively matches source field patterns
against checked types without covariance or changing list fixedness.
