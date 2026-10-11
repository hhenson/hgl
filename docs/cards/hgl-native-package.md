# Card: hgl-native-package

Inert complete native package records in hgl-source. Module budget 80 lines within
the unchanged aggregate; uses native_descriptor token readers and portable Types
Scalar/Function data. It stores no callbacks or loaded provider state.

Public Descriptor records module, provider, fingerprint, optional image location,
scalars and functions. verify_image compares the exact checked provider contract.
header parses version-1 package identity and records; image resolves an optional
Rust target path relative to the descriptor using path data without opening code.
validate_aliases requires one exact capability contract for each canonical identity.
Semantic Library loading and checking retain one validated representation.

`Descriptor.markers` retains the optional explicit target identifier of each
scalar declaration. It is separate from portable canonical identity/capabilities;
changing an import alias never changes the ordinary nominal identity. Missing
markers remain representable for descriptor-only declaration checking.
