# Card: hgl-struct-names

Resolve finite struct declaration names and export visibility. Dependencies:
hgl-library and hgl-source; budget 160 source lines. Compile-time only.

Public `identity(&Library, module: &str, name: &str) -> (String, String)` resolves
imported identities. `declaration<'a>(&'a Library, module: &str, name: &str) ->
Result<Option<&'a Decl>, String>` applies the existing local/import lookup,
duplicate declaration, qualification and visibility rules.

`exported_fields(&Library, &Decl) -> Result<(), String>` validates the
declaration-owned field and generic-argument export closure. Private types
supplied by a caller do not become exported merely through specialization.
Resolution and generic occurrence validation use the same declaration lookup.

Acceptance: existing imported-struct, visibility and generic specialization
tests, including local and imported declarations with the same name.

Expose enum_declaration with the same signature and visibility rules as
declaration, selecting Role::Enum. Exported field closure includes enum names.

A local nominal declaration takes precedence over an imported name across both
struct and enum kinds; changing the requested kind cannot bypass that shadow.
