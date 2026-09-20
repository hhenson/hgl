//! A description brought to life (specification: Graph, Part 1,
//! "Behaviour").

use std::collections::HashSet;

use hgl_kernel::{Graph, NodeSlot};
use hgl_store::{InputId, Store};
use hgl_types::{NodeId, NodeType};

use crate::registry::Ports;
use crate::{BuildError, GraphDescription, Registry, check_edge, check_scalars, index};

/// Build a graph from a description: resolve every implementation, create the
/// nodes in order, bind every edge. All or nothing (GRF-9). The node at
/// `description.nodes[i]` is `NodeId(i)` in the graph: that is how a tool
/// finds a node again (`Graph::node`).
///
/// Everything a description can get wrong is found before the store is
/// touched, so such a failure adds nothing to it. A node whose own build
/// fails is different: every port already made stays — each earlier node's,
/// and whatever the failing node took — unbound and unreachable. The graph
/// is all or nothing (GRF-9); the store is not yet. hgraph rolls its
/// storage back, and P1 has no way to: releasing a graph's ports waits for
/// P4, where they are a range the store can give back.
///
/// Errors: an implementation is unknown (GRF-9); a node's scalars do not
/// conform (GRF-8); an edge does not run forward (GRF-4), names an input or
/// output that does not exist, joins different types (GRF-7), or binds an
/// input already bound (GRF-6); a node's build fails.
pub fn instantiate(
    description: &GraphDescription,
    registry: &Registry,
    store: &mut Store,
) -> Result<Graph, BuildError> {
    let nodes = &description.nodes;
    let mut implementations = Vec::with_capacity(nodes.len());
    let mut types = Vec::with_capacity(nodes.len());
    for node in nodes {
        let implementation = registry.find(&node.implementation)?;
        check_scalars(&implementation.node_type, node)?;
        implementations.push(implementation);
        types.push(&implementation.node_type);
    }
    let mut bound = HashSet::new();
    for &edge in &description.edges {
        // The order is the rank order (GRF-3), so it must already put every
        // producer first.
        if edge.source_node >= edge.target_node {
            return Err(BuildError::Cycle);
        }
        if edge.target_node as usize >= nodes.len() {
            let node = edge.target_node.to_string();
            let input = edge.target_input.to_string();
            return Err(BuildError::unknown_input(&node, &input));
        }
        check_edge(nodes, &types, edge, &mut bound)?;
    }

    let mut slots = Vec::with_capacity(nodes.len());
    let mut inputs = Vec::with_capacity(nodes.len());
    let mut outputs = Vec::with_capacity(nodes.len());
    for (position, (node, implementation)) in nodes.iter().zip(implementations).enumerate() {
        let node_type = &implementation.node_type;
        let mut ports = Ports {
            store,
            node: NodeId(index(position)),
            node_type,
            description: node,
            inputs: vec![None; node_type.inputs.len()],
            output: None,
        };
        let built = (implementation.build)(&mut ports)?;
        let (node_inputs, output) = ports.into_ids();
        slots.push(NodeSlot {
            node: built,
            node_type: node_type.clone(),
            label: node.label.clone(),
            required: required(node_type, &node_inputs),
        });
        inputs.push(node_inputs);
        outputs.push(output);
    }
    for edge in &description.edges {
        let source = edge.source_node as usize;
        // `check_edge` found an output on every source's node type; this is
        // the one the source's build left behind.
        let no_output = || BuildError::no_output(&nodes[source].label);
        let output = outputs[source].ok_or_else(no_output)?;
        let input = inputs[edge.target_node as usize][edge.target_input as usize];
        store.bind(input, output).map_err(BuildError::Bind)?;
    }
    Ok(Graph::new(description.label.clone(), slots))
}

/// NOD-2: the inputs that must be valid before `eval` is called.
fn required(node_type: &NodeType, inputs: &[InputId]) -> Vec<InputId> {
    match &node_type.valid_inputs {
        None => inputs.to_vec(),
        Some(valid) => valid.iter().map(|&position| inputs[position]).collect(),
    }
}
