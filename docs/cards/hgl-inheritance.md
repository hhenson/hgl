# Card: hgl-inheritance

Scoped finite single-inheritance patterns for ordinary constructors and closed
abstract family membership. Depends on source, library and struct-names; budget
300 source lines. All work occurs during source checking.

Pattern is Parameter(root parameter) or Named(declaration module, base, arguments).
resolve substitutes checked arguments through a caller-owned type resolver;
infer propagates a caller-decomposed actual application into root bindings.
schema(library,decl) returns RequiredStruct and parallel scoped field patterns,
flattened ancestor first; introducing defaults and optional indices are retained.
ancestors(library,decl) returns scoped transitive applications nearest first.
Every resolved ancestor declaration must be abstract, including imported aliases
and generic applications. Concrete bases, cycles, incomplete ancestor applications
and field redeclarations are diagnosed.
No field projection, dispatch, cast or runtime nominal lookup is introduced.

Fixed-list field patterns retain the size expression as scoped bound syntax,
not as a nominal type argument. Pattern substitution resolves only the element;
family field resolution and inference delegate the reconstructed list type to
ordinary checked constant-size normalization in the ancestor's module.
