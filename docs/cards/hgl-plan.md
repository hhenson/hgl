# Card: hgl-plan

Status: accepted infrastructure target; GRF-1–10 and the validated description traces.

Owns plain graph descriptions and their structural checks. Uses `hgl-types`
and `hgl-store::BindError`; holds no runtime object. Budget: 500 lines.

- `GraphDescription { label, nodes, edges }` and
  `NodeDescription { implementation, label, scalars, children }` remain owned data.
- `Step::{Field(String), Index(usize), Key}` describes an endpoint path. `Key`
  is permitted only in a keyed child's owner-input boundary; its actual key is
  supplied when that child is instantiated.
- `InputPort { node, input, path }`, `OutputPort { node, path }` and
  `Edge { source: OutputPort, target: InputPort }` use graph-local positions.
- `ChildDescription { graph, inputs, output, keyed }` owns a reusable child
  template. `Boundary { source_input, source_path, target }` binds an owner's
  input view to a child input. `output` is an optional child output path;
  `keyed` makes it one dictionary member rather than the whole owner output.
- `BuildError` retains existing variants and borrowed-name constructors
  (`unknown_input`, `no_output`, `unknown_scalar`, `wrong_type`, `bound_twice`),
  adding `InvalidPath`, `InvalidChildren` and `MissingKey`.
- `Catalog::node_type` supplies resolved signatures by implementation name.
  `validate(description, catalog)` checks the complete recursive template;
  `input_type(port, description, catalog)` and `output_type` resolve endpoints.
  `check_boundaries(template, source_types, catalog)` checks an instance interface.
- `check_scalars`, `check_edge`, `project`, `check_shape` and `index` are the
  shared construction-time checks. They never allocate endpoint storage.

Types are recursive and fully resolved. Edge paths contain only fixed fields
and indices. Reject invalid paths, incompatible types, overlapping whole/child
bindings, unresolved implementations and malformed child boundaries before
instantiation. Distinct sibling bindings are valid. A REF source may feed its
exact target type; all other connections currently require equal ordered shapes.
This is an implementation limit, not WIR-6/WIR-15 compatibility. Reordered
bundles and recursive REF alternatives need a checked binding plan before
acceptance can widen; see the [wiring review](../compiler/wiring-review.md).

Required rejection cases: wrong field/index/kind, nested shape mismatch,
whole/descendant overlap in either order, duplicate child fields, out-of-range
nodes, child boundary/internal-edge overlap and missing runtime key. No rejected
loaded description may add endpoints. Reusing a template preserves its data and
creates independent state and ports. Names and paths are resolved only while
constructing graphs.

`Builder` and `NodeRef` live here and are re-exported by hgl-describe. The
builder accepts a `Catalog` and performs the same checks as loaded descriptions.
`Catalog` supports diagnostic `Debug` formatting.

Shape validation and keyed child paths preserve KeyedDictionary's exact ordinary
key type, and accept KeyedSet as a scalar-membership shape. Edge compatibility
still requires the complete exact schema, including nominal key identity.
