# Card: hgl-enums

Checked nominal enum declarations and qualified member references. Uses
hgl-library, hgl-source and hgl-struct-names; budget 250 source lines.
No third-party dependencies, provider state or runtime lookup.

Public functions: validate(&Library) -> Result<(), String>,
resolve(&Library, module: &str, name: &str) -> Result<Option<EnumType>, String>,
member(&Library, module: &str, name: &str) -> Result<Option<Literal>, String>,
default(&Library, module: &str, &Expr) -> Result<ParsedLiteral, String>,
check_call(&Library, module: &str, name: &str, &[(Option<String>, Expr)])
-> Result<(), String>.

Validate distinct member names and assigned signed i64 numbers, zero-based initial
numbering, checked automatic successors and explicit resets. Parse the signed
minimum directly through source constant parsing. Member references preserve
canonical module/declaration identity and require exact declared names. Respect
imports and export visibility. Defaults retain existing fixed scalar and
contextual temporal behavior, adding only qualified enum members. Unknown members
and unsupported construction forms are explicit diagnostics.

Acceptance: min/max, negative and automatic numbering, explicit reset after max,
duplicate names/numbers/declarations, overflow, import identity and private export rejection.

check_call diagnoses enum construction and enum-type enumeration as unsupported
by the declared-member publication profile before ordinary call resolution.
