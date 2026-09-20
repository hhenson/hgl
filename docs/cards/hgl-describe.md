# Card: hgl-describe

## Purpose

The graph description, a builder for writing one by hand, and instantiation.
It is the prototype's stand-in for wiring: there is no compiler, so a person
describes a graph in Rust. The description itself is plain data
(specification: Graph, Part 1) and could be written out unchanged.

## May use

`hgl-types`, `hgl-store`, `hgl-kernel`.

## What building a node looks like

The second half of a node (the first is its `Node` impl, in
[hgl-kernel](hgl-kernel.md)). Names are resolved to handles here, once, and
never again.

```rust
impl Buildable for Sum {
    fn node_type() -> NodeType { /* name "sum"; inputs lhs, rhs: TS<i64>; output TS<i64> */ }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self { lhs: ports.input("lhs")?, rhs: ports.input("rhs")?, out: ports.output()? })
    }
}
```

## Surface

```rust
// --- the description: plain data, no closures, no pointers ---
#[derive(Debug, Clone, PartialEq)]
pub struct GraphDescription { pub label: String, pub nodes: Vec<NodeDescription>, pub edges: Vec<Edge> }
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDescription {
    /// The name an implementation is registered under (GRF-9).
    pub implementation: String,
    pub label: String,
    pub scalars: Vec<(String, ScalarValue)>,
}
/// P1 binds whole `TS` inputs to whole `TS` outputs, so there are no paths yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge { pub source_node: u32, pub target_node: u32, pub target_input: u32 }

// --- what a node supplies so that it can be built ---
/// Registered under `node_type().name` (GRF-9).
pub trait Buildable: Node + Sized {
    fn node_type() -> NodeType;
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError>;
}
/// A node's typed handles and scalars, by name, while it is being built.
pub struct Ports<'a> { /* private */ }
impl Ports<'_> {
    pub fn input<T: Scalar>(&mut self, name: &str) -> Result<In<T>, BuildError>;
    pub fn output<T: Scalar>(&mut self) -> Result<Out<T>, BuildError>;
    pub fn scalar<T: Scalar>(&self, name: &str) -> Result<T, BuildError>;
}

#[derive(Default)]
pub struct Registry { /* name -> node type and constructor */ }
impl Registry {
    pub fn new() -> Self;
    /// Errors if the name is taken, or the node type is not well formed.
    pub fn register<N: Buildable>(&mut self) -> Result<(), BuildError>;
    pub fn node_type(&self, implementation: &str) -> Option<&NodeType>;
}

// --- the hand builder: stands in for wiring ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct NodeRef(u32);
pub struct Builder<'r> { /* private */ }
impl<'r> Builder<'r> {
    pub fn new(label: &str, registry: &'r Registry) -> Self;
    pub fn node(&mut self, implementation: &str, scalars: &[(&str, ScalarValue)]) -> Result<NodeRef, BuildError>;
    pub fn connect(&mut self, source: NodeRef, target: NodeRef, input: &str) -> Result<(), BuildError>;
    /// Ranks the nodes as hgraph does (`build_ranked_graph`): Kahn's
    /// algorithm with a first-in-first-out queue, seeded with the nodes that
    /// have no producers in insertion order. Refuses a cycle. The edges are
    /// sorted by target, then input position, *before* ranking as well as
    /// after: that is the order hgraph reaches a producer's consumers in, and
    /// a first-in-first-out queue alone does not reproduce it. `NodeRef`s
    /// given out are not valid in the result: its nodes are in rank order.
    pub fn finish(self) -> Result<GraphDescription, BuildError>;
}

/// Build a graph from a description: resolve every implementation, create the
/// nodes in order, bind every edge. All or nothing (GRF-9). The node at
/// `description.nodes[i]` is `NodeId(i)` in the graph: that is how a tool
/// finds a node again (`Graph::node`).
pub fn instantiate(description: &GraphDescription, registry: &Registry, store: &mut Store)
    -> Result<Graph, BuildError>;

#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    UnknownImplementation(String), DuplicateImplementation(&'static str),
    UnknownInput { node: String, input: String }, NoOutput { node: String },
    UnknownScalar { node: String, scalar: String }, WrongType { node: String, what: String },
    InputBoundTwice { node: String, input: String }, Cycle, Bind(BindError),
    /// Refused by `register`: an active or valid position past the last
    /// input, or two inputs with one name.
    InvalidNodeType { node: &'static str, what: String },
}
```

*Revised after the build (2026-09-20). What the card left open, as built:*

- *A node's `build` takes the ports it reads. Every declared port it does
  not take is still made, from its declared type, once `build` returns — so
  an edge or a required input always has an id, and a trigger input needs
  no field.*
- *`finish` ranks as hgraph's C++ does (`graph_wiring.cpp`,
  `build_ranked_graph`): first-in-first-out, seeded in insertion order — so
  the same wiring gives the same node ids, start and stop order and
  evaluation order here as there. (The first build chose the
  lexicographically smallest order, which a fresh review showed differs
  from the reference.) Edges are sorted by target and input position, as
  hgraph emits them, so two builds of one graph compare equal.*
- *`instantiate` re-checks everything `Builder` checks — scalars conform
  (none missing, none twice, none undeclared, each of its type), edges run
  forward and name what exists, types match, no input bound twice — before
  it touches the store (GRF-7: a loaded description must establish it
  again). So a bad description leaves the store as it was; `Bind(..)` is
  the store's safety net and cannot be reached through a checked
  description. A node whose own `build` fails leaves the ports already made
  — every earlier node's, and whatever the failing node took — unbound and
  unreachable: the graph is all or nothing (GRF-9), the store is not yet.
  hgraph rolls its storage back; here that waits for P4, where a graph's
  ports become a range the store can release. Until then it is a leak, and
  only for a store that outlives many failed instantiations.*
- *`register` checks what the compiler cannot: positions in `active_inputs`
  and `valid_inputs` exist, and input names are distinct. Without it an
  out-of-range valid position panicked in `instantiate`, and an out-of-range
  active position silently made nothing active.*
- *A node states its name once, in `node_type()`; a separate
  `Buildable::NAME` was dropped, because nothing but a check could keep the
  two in step. The duplicate-name refusal is now the only guard on that
  name: a node type given another node's name by accident — copied from its
  neighbour and not renamed — comes back as `DuplicateImplementation`, which
  is the right error but reads as being about the registry, not the type.*
- *`Builder::node` labels a node with its implementation name. Errors in a
  stored description, which has positions and no names, carry the positions
  as text: an edge that does not run forward is `Cycle`; an edge to a node
  that does not exist is `UnknownInput` with the position as its name. A
  scalar missing or given twice is `UnknownScalar`. Taking the output twice
  is `NoOutput`.*
- *`Registry`, `Ports` and `Builder` derive `Debug` (a workspace lint).*

## Rules

GRF-1, 2, 3, 4, 6, 7, 8, 9. (GRF-5, push sources, and GRF-10, child graphs,
are later slices.)

## Speed

Nothing here runs during a tick. Instantiation may allocate freely in P1;
its cost is measured in P4. What matters now is what it leaves behind: every
name has become a handle, so `eval` never looks anything up.

## Budget

500 lines. *(Revised 2026-09-20 by the card's author, from the measurement:
450 held the first build only because errors were routed through one-line
constructors and values unpacked in tuple `let`s — terse, which this project
does not want. Written plainly, and with `register` checking a node type,
the crate is about 480. The check earned its place on its first run by
refusing a malformed node type in the crate's own tests.)*

## Done when

Unit tests, each naming its rule: `finish` orders producers before consumers,
and of the nodes whose producers are all ranked takes them first in, first
out (so a source added late is still ranked before a consumer added early);
two builds of one graph compare equal; a cycle is rejected; an unknown
implementation, input or scalar, a wrong type, and an input bound twice are
each rejected with the matching error; an unresolved implementation yields no
graph at all; the same description instantiated twice gives two independent
graphs that tick identically; the description compares equal to a copy of
itself after instantiation.

## Mutants

Each must make a test fail.

- `finish` keeps insertion order and does no topological sort.
- A cycle is accepted.
- `connect` does not check that the types match.
- An input may be bound twice.
- An unknown implementation yields a graph with that node missing.
- `instantiate` changes the description.
- Two graphs from one description share a store slot.
- A node's id is not its position in the description (its store handles
  are owned by another id).
- A port the build does not take is not made, or made with the wrong type
  or activity.
- `valid_inputs` / `active_inputs` positions are read as position 0.
- A second registration overwrites the first, whether or not it reports the
  duplicate.
- A refused description has added ports to the store.
- The edges are not sorted before ranking (so a producer's consumers are
  reached in the order the edges were made).
- `register` accepts a node type whose `what` the tests pin: a position past
  the last input, or two inputs of one name.

Known equivalent, so no test can catch it: an *untaken* output owned by the
wrong node. Nothing writes it, and an owner is read only by `set` (TS-21).
- `finish` breaks ties by lowest insertion position rather than first in,
  first out.
- An untaken input is not active, or not required, under the defaults
  (`None`).
- `instantiate` checks scalars only when the node type declares some; its
  node-range check is off by one.
- `register` accepts a position past the last input, or two inputs of one
  name.
