# hgl-name-check

Status: accepted

Resolve parsed callable references before deferred generic and required-constant
body specialization. `Scope` carries inferred types, generic constructors,
callable requirements and lexical service bindings. `expression` traverses the
AST, resolves module/import identities and visible declaration candidates, and
retains intrinsic call candidates for later operand/phase validation. It neither
fabricates generic/configuration values nor evaluates expressions. Qualified enum values use the existing enum-member resolver. Bare local value
names and property labels remain ordinary value/specialization checking's work.
`visible` is the shared callable candidate visibility rule used by ordinary
program overload selection, including module test-context shadowing.

Acceptance: excluded generic/required-const dependencies fail ordinary admission;
valid generic calls, local value shadowing, test-context overloads and intrinsic
calls retain deferred specialization. Budget 250; existing source/library/name
index dependencies only.

`signature`, `structure` and `type_name` resolve nominal names and recursively
walk parsed bound expressions at signatures, fields and local annotations.
Defaults and native requirement types use this same walk. Shape specialization
and constant evaluation remain with the ordinary checker. Uses existing enum
and bound-expression parser crates.
