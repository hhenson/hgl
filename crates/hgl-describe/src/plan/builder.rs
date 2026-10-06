//! Writing a description by hand: the prototype's stand-in for wiring.

use std::collections::VecDeque;

use hgl_types::{NodeType, ScalarValue};

use crate::plan::{
    BuildError, Catalog, ChildDescription, Edge, GraphDescription, InputPort, NodeDescription,
    OutputPort, Step, check_edge, check_scalars, index,
};

/// A node added to a [`Builder`], by the order it was added in. Only that
/// builder knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRef(u32);

/// Writes a [`GraphDescription`]: nodes by implementation name, edges by
/// input name. Each call checks what it adds; [`Builder::finish`] ranks.
#[derive(Debug)]
pub struct Builder<'r> {
    registry: &'r dyn Catalog,
    label: String,
    /// In the order they were added, until `finish` ranks them.
    nodes: Vec<NodeDescription>,
    /// Each node's type, so that `connect` needs no lookup by name.
    types: Vec<&'r NodeType>,
    edges: Vec<Edge>,
    /// The (node, input) of every edge so far (GRF-6).
    bound: Vec<InputPort>,
}

impl<'r> Builder<'r> {
    /// An empty graph called `label`, of implementations from `registry`.
    pub fn new(label: &str, registry: &'r dyn Catalog) -> Self {
        Self {
            registry,
            label: label.to_owned(),
            nodes: Vec::new(),
            types: Vec::new(),
            edges: Vec::new(),
            bound: Vec::new(),
        }
    }

    /// Add a node of `implementation`, labelled with that name.
    ///
    /// Errors: no such implementation (GRF-9); the scalars do not conform to
    /// its node type (GRF-8).
    pub fn node(
        &mut self,
        implementation: &str,
        scalars: &[(&str, ScalarValue)],
    ) -> Result<NodeRef, BuildError> {
        let node_type = self
            .registry
            .node_type(implementation)
            .ok_or_else(|| BuildError::UnknownImplementation(implementation.into()))?;
        let node = NodeDescription {
            implementation: implementation.to_owned(),
            children: Vec::new(),
            label: implementation.to_owned(),
            scalars: scalars
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect(),
        };
        check_scalars(node_type, &node)?;
        let added = NodeRef(index(self.nodes.len()));
        self.nodes.push(node);
        self.types.push(node_type);
        Ok(added)
    }

    /// Bind `target`'s input called `input` to `source`'s output.
    ///
    /// Errors: `target` has no such input; `source` has no output; their
    /// types differ (GRF-7); the input is bound already (GRF-6).
    pub fn connect(
        &mut self,
        source: NodeRef,
        target: NodeRef,
        input: &str,
    ) -> Result<(), BuildError> {
        self.connect_path(source, Vec::new(), target, input, Vec::new())
    }

    /// Connect checked fixed descendants instead of whole ports.
    pub fn connect_path(
        &mut self,
        source: NodeRef,
        source_path: Vec<Step>,
        target: NodeRef,
        input: &str,
        target_path: Vec<Step>,
    ) -> Result<(), BuildError> {
        let inputs = &self.types[target.0 as usize].inputs;
        let Some(position) = inputs.iter().position(|&(name, _)| name == input) else {
            let label = &self.nodes[target.0 as usize].label;
            return Err(BuildError::unknown_input(label, input));
        };
        let edge = Edge {
            source: OutputPort {
                node: source.0,
                path: source_path,
            },
            target: InputPort {
                node: target.0,
                input: index(position),
                path: target_path,
            },
        };
        check_edge(&self.nodes, &self.types, &edge, &mut self.bound)?;
        self.edges.push(edge);
        Ok(())
    }

    /// Set the reusable child templates of one owner node.
    pub fn children(
        &mut self,
        node: NodeRef,
        children: Vec<ChildDescription>,
    ) -> Result<(), BuildError> {
        if children.len() != self.types[node.0 as usize].child_graphs {
            return Err(BuildError::InvalidChildren(
                self.nodes[node.0 as usize].label.clone(),
            ));
        }
        self.nodes[node.0 as usize].children = children;
        Ok(())
    }

    /// Ranks the nodes as hgraph does (`build_ranked_graph`): Kahn's
    /// algorithm with a first-in-first-out queue, seeded with the nodes that
    /// have no producers in insertion order. Edges come out sorted by target,
    /// then input position, so two builds of one graph compare equal.
    /// `NodeRef`s given out are not valid in the result: its nodes are in
    /// rank order.
    ///
    /// Errors: the edges form a cycle (GRF-4).
    pub fn finish(self) -> Result<GraphDescription, BuildError> {
        let mut edges = self.edges;
        // hgraph reaches a producer's consumers by walking the consumers in
        // insertion order and each one's inputs in order. Sorting first puts
        // the edges in that order, which is the order the ranking then sees.
        edges.sort_unstable_by(|a, b| a.target.cmp(&b.target));
        let ranks = ranks(self.nodes.len(), &edges)?;
        let mut ranked: Vec<(u32, NodeDescription)> =
            ranks.iter().copied().zip(self.nodes).collect();
        ranked.sort_unstable_by_key(|&(rank, _)| rank);
        for edge in &mut edges {
            edge.source.node = ranks[edge.source.node as usize];
            edge.target.node = ranks[edge.target.node as usize];
        }
        edges.sort_unstable_by(|a, b| a.target.cmp(&b.target));
        let description = GraphDescription {
            label: self.label,
            nodes: ranked.into_iter().map(|(_, node)| node).collect(),
            edges,
        };
        crate::plan::validate(&description, self.registry)?;
        Ok(description)
    }
}

/// Each node's rank, by Kahn's algorithm as hgraph's `build_ranked_graph`
/// runs it (GRF-3, GRF-4): every node with no producer is ready, in
/// insertion order, and a node made ready joins the back of the queue.
fn ranks(count: usize, edges: &[Edge]) -> Result<Vec<u32>, BuildError> {
    let mut consumers = vec![Vec::new(); count];
    let mut unranked_producers = vec![0_usize; count];
    for edge in edges {
        consumers[edge.source.node as usize].push(edge.target.node);
        unranked_producers[edge.target.node as usize] += 1;
    }
    let mut ready: VecDeque<u32> = (0..index(count))
        .filter(|&node| unranked_producers[node as usize] == 0)
        .collect();
    let mut ranks = vec![0; count];
    let mut next = 0;
    while let Some(node) = ready.pop_front() {
        ranks[node as usize] = next;
        next += 1;
        for &consumer in &consumers[node as usize] {
            unranked_producers[consumer as usize] -= 1;
            if unranked_producers[consumer as usize] == 0 {
                ready.push_back(consumer);
            }
        }
    }
    // A node never ranked waits on a producer that waits on it.
    if next != index(count) {
        return Err(BuildError::Cycle);
    }
    Ok(ranks)
}
