# Card: hgl-library

Index source modules, parts, imports, operator instances and test contexts.
Uses `hgl-source` and `hgl-struct-declarations`; budget 500 lines. Allocation happens during compilation.

Surface: `load(&[(String,String)]) -> Result<Library,String>`;
`Library { declarations, root, imports, instances }`;
`Decl { module, name, role, tokens, source, doc, test_only }`, `Decl::signature`;
`Role::{Function,Implementation,Operator,Native,Test}`;
`Parameter { name, ty, constant, default }`;
`Signature { generics, parameters, result, body, value_function, throws,
requirement, type_domain }`. Native requirements retain the native name,
argument types and result. `type_domain: Option<(String, Vec<String>)>` retains
`requires T in {...}` as the generic name and distinct supported type names;
resolution checks the inferred type against this finite domain.

Parts are unique. An unnamed test context permits private functions and named
tests, joining the module's test scope. Imports, exports, native/type/operator
declarations and nested contexts are rejected there. Resolution and target
selection belong to `hgl-program`; indexing does not execute a body.

Acceptance: unchanged multi-part library; forward helper references across
contexts; invalid context members; production helper isolation.

`Role::Struct` indexes ordinary struct declarations without treating them as
functions. `Decl::required_struct() -> Result<RequiredStruct, String>`
parses finite nonrecursive fields, preserving source type names and checked
non-null scalar literal defaults (including contextual calendar recipes) for frontend resolution. Duplicate fields and unsupported generic declarations are diagnosed.

`RequiredStruct { generics, fields, defaults, type_domain }` and `Decl::required_struct`
retain ordered type parameters and the supported finite type-domain constraint.
`defaults` pairs declaration field indices with fixed scalar literals. Defaults
use the existing source fixed-expression evaluator; other constant expressions
remain unsupported. Concrete specialization checks each default against its field
type even when construction supplies that field or constructs only a delta.
Type parameters are distinct; const parameters, generic defaults and unsupported
constraints are diagnosed explicitly.

Explicit instantiate arguments preserve complete nested type syntax (including
contextual delta<T>); wildcard `_` remains a retained generic argument. The
binder resolves their declaration-owned canonical identity before matching an
implementation, never comparing only a scalar/shape display name.

`Library::type_sizes` caches checked declaration-owned constant size expressions
by module and expression during source compilation. It contains only canonical
i64 values, is populated by the normal ordinary expression checker/evaluator,
and never reaches runtime shape storage or emitted lookup code.

StructSchema.defaults stores Vec<(usize, hgl_source::ParsedLiteral)> so fixed
ordinary literals and contextual scalar recipes share the existing default
profile. Type-check each against the substituted canonical field type. Omitted
fields construct/retain defaults in declaration order after supplied fields; an
explicit field suppresses its default. No provider call occurs during indexing.

Role::Enum indexes enum declarations as nominal types, including export visibility.
RequiredStruct.defaults now stores Expr, retaining a qualified enum member name
until declaration-owned resolution. Other defaults retain the existing closed
scalar and contextual temporal profile.

`RequiredStruct::optional: Vec<usize>` records fields introduced with a null
default; `defaults` retains that Expr::Null along with existing scalar defaults.
The parser admits optional ordinary fields without new default-expression
operations. Field types remain their declared types.

RequiredStruct parsing is owned by hgl-struct-declarations. abstract_type and
parent retain abstract declaration status and one source ancestor application;
Role::Struct indexes both concrete and abstract declarations. Multiple parents
remain explicitly unsupported. Semantic inheritance/closure checking is separate.
