# Card: hgl-library

Index source modules, parts, imports, operator instances and test contexts.
Uses `hgl-source`; budget 500 lines. Allocation happens during compilation.

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
parses the required-field subset, preserving source type names for
frontend resolution. Duplicate fields, defaults, optional fields, inheritance
and unsupported generic declarations are diagnosed when this subset is requested.

`RequiredStruct { generics, fields, type_domain }` and `Decl::required_struct`
retain ordered type parameters and the supported finite type-domain constraint.
Type parameters are distinct; const parameters, defaults, optional fields,
inheritance and unsupported constraints are diagnosed explicitly.
