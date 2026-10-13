# Card: hgl-graph-assemblies

Status: implemented fixed nominal connection assembly leaf.

The graph_assemblies module in hgl-semantics uses checked IR and hgl-source.
Public constructor(Value, &mut Plan) -> Result<Value, String> leaves ordinary
constructors unchanged and reduces complete nominal constructors with temporal
children to a checked assembly node. Field sources are addressed by declared
identity, retaining duplicate endpoint sources. Node-hook construction keeps
ordinary retention rules. Mixed ordinary/temporal children, optional fields,
and non-assembly REF-producing child connections remain implementation gaps, rather than
language exclusions. Nested statically assembled nominal fields flatten to
their original endpoint paths during description construction.

The internal graph_assemblies module in hgl-rust uses checked IR and hgl-source.
shape(Node) recognizes the checked assembly opcode, node(index, shape) emits a
passive fixed input with a REF output, and connections(Plan, Node, index) emits
fixed field path edges. connections_list(Plan, Node) returns original source
indices and checked emitted Field/Index steps for scoped description lowering.
hgl-describe construction interns that input designation from
the original output identities before startup. Evaluation publishes only its
prepared designation; source payloads, delta omissions and timestamps remain
in their existing endpoints. reference_routes expands each assembly field to
its actual endpoint and consumer child view, including duplicate subscriptions.

Acceptance: source admission and scalar/node constructor controls, executed
sparse/equal/duplicate child ticks, projection and held REF controls, cold child
endpoint identity checks, and actual CountingAllocator measurement beginning
with the first successful cycle in debug and ci-release profiles.

Public reference_list(Vec<Value>, &mut Plan) -> Result<Value,String> reduces
uniform temporal graph entries to a fixed REF-child connection list. Ordinary
ports receive stable designation-forwarding nodes; existing REF outputs stay
opaque carrier connections. Fixed-list paths use declared numeric positions.
Flattening stops at REF child boundaries, including nominal assemblies and
conditional owner carriers. Ordinary consumers expand designated targets;
REF-child consumers reserve the carrier endpoints themselves.
