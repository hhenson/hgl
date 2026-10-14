# hgl-name-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `name_check` module of `hgl-semantics` (`crates/hgl-semantics/src/name_check.rs`); `cargo xtask ci` reports its size and enforces its "may use" list.

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

Scope delegates source runtime provenance to tuple_flow Facts, including ordinary
aliases, assignments and injected payload capabilities. Lambda parameters shadow service and runtime names.
Constant Tuple call boundaries use hgl-tuple-phase before deferred specialization.

Scope.alias accepts an explicit runtime-binding flag. State/cache identities and
assignment dependence remain runtime facts even when a fixed initializer exists.

Test-local struct and enum identities are visible only in their module test scope.
Signature, field, local and lambda annotations, constructors and qualified enum
values enforce this before deferred specialization, including unused generic
production bodies. Nominal schema identity remains module-wide; no runtime lookup
or alternate test schema is introduced.

Built-in scalar and type-constructor spellings retain ordinary type-resolution
precedence; hidden test nominals cannot intercept production annotations.

`capture_intrinsic(&Library,module,test_scope) -> bool` preserves visible
ordinary callable shadowing. Builtin capture name admission checks its complete
selected callable as an ordinary call with the remaining written arguments,
including qualified imports, without evaluating or bypassing selection.
