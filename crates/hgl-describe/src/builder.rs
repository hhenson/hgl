//! Writing a description by hand: the prototype's stand-in for wiring.

use std::collections::{HashSet, VecDeque};

use hgl_types::{NodeType, ScalarValue};

use crate::{
    BuildError, Edge, GraphDescription, NodeDescription, Registry, check_edge, check_scalars, index,
};

/// A node added to a [`Builder`], by the order it was added in. Only that
/// builder knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRef(u32);

/// Writes a [`GraphDescription`]: nodes by implementation name, edges by
/// input name. Each call checks what it adds; [`Builder::finish`] ranks.
#[derive(Debug)]
pub struct Builder<'r> {
    registry: &'r Registry,
    label: String,
    /// In the order they were added, until `finish` ranks them.
    nodes: Vec<NodeDescription>,
    /// Each node's type, so that `connect` needs no lookup by name.
    types: Vec<&'r NodeType>,
    edges: Vec<Edge>,
    /// The (node, input) of every edge so far (GRF-6).
    bound: HashSet<(u32, u32)>,
}

impl<'r> Builder<'r> {
    /// An empty graph called `label`, of implementations from `registry`.
    pub fn new(label: &str, registry: &'r Registry) -> Self {
        Self {
            registry,
            label: label.to_owned(),
            nodes: Vec::new(),
            types: Vec::new(),
            edges: Vec::new(),
            bound: HashSet::new(),
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
        let node_type = &self.registry.find(implementation)?.node_type;
        let node = NodeDescription {
            implementation: implementation.to_owned(),
            label: implementation.to_owned(),
            scalars: scalars
                .iter()
                .map(|&(name, value)| (name.to_owned(), value))
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
        let inputs = &self.types[target.0 as usize].inputs;
        let Some(position) = inputs.iter().position(|&(name, _)| name == input) else {
            let label = &self.nodes[target.0 as usize].label;
            return Err(BuildError::unknown_input(label, input));
        };
        let edge = Edge {
            source_node: source.0,
            target_node: target.0,
            target_input: index(position),
        };
        check_edge(&self.nodes, &self.types, edge, &mut self.bound)?;
        self.edges.push(edge);
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
        edges.sort_unstable_by_key(|edge| (edge.target_node, edge.target_input));
        let ranks = ranks(self.nodes.len(), &edges)?;
        let mut ranked: Vec<(u32, NodeDescription)> =
            ranks.iter().copied().zip(self.nodes).collect();
        ranked.sort_unstable_by_key(|&(rank, _)| rank);
        for edge in &mut edges {
            edge.source_node = ranks[edge.source_node as usize];
            edge.target_node = ranks[edge.target_node as usize];
        }
        edges.sort_unstable_by_key(|edge| (edge.target_node, edge.target_input));
        Ok(GraphDescription {
            label: self.label,
            nodes: ranked.into_iter().map(|(_, node)| node).collect(),
            edges,
        })
    }
}

/// Each node's rank, by Kahn's algorithm as hgraph's `build_ranked_graph`
/// runs it (GRF-3, GRF-4): every node with no producer is ready, in
/// insertion order, and a node made ready joins the back of the queue.
fn ranks(count: usize, edges: &[Edge]) -> Result<Vec<u32>, BuildError> {
    let mut consumers = vec![Vec::new(); count];
    let mut unranked_producers = vec![0_usize; count];
    for edge in edges {
        consumers[edge.source_node as usize].push(edge.target_node);
        unranked_producers[edge.target_node as usize] += 1;
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
