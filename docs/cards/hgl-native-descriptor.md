# Card: hgl-native-descriptor

Pure inert descriptor record syntax, using the existing source lexer and Cursor.
No runtime/provider dependency; module budget 140 lines within the unchanged
hgl-source crate budget. Source indexing never calls provider code.

Public Scalar stores canonical identity and declared capabilities. Function
stores readonly parameter names/types, owning result type, exception policy,
target symbol, phases and effects. Public version, text, scalar and function
parse the existing version-1 data sidecar records. Package module/provider
binding, canonical type comparison and alias validation remain semantic work.

Required-constant execution, signal payloads, borrowing, mutation, generic/default
parameter forms and blocking evaluation remain rejected. Fields retain the source
metadata needed for exact checking; parsing never fabricates a runtime value.
