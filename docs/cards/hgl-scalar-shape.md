# Card: hgl-scalar-shape

The scalar_shape source policy supplies shared cold builtin storage metadata. Initial module budget 80 lines; no internal module dependencies.

Public surface: Ty::scalar_kind()->Option<hgl_types::ScalarType>. It identifies existing builtin scalar representations. Nominal enums and Any use their own exact ordinary markers and return None. Structural/temporal wrappers retain their existing normalized identity. This method adds no source native type declaration or conversion.

Acceptance: existing scalar layouts/conversion metadata and typed compiled Any codecs share one exhaustive builtin representation selection.

`Ty::owning_scalar` supplies the cold variable-storage classification shared by structural ownership admission, including Any.
