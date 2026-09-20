//! The graph description, a builder for writing one by hand, and
//! instantiation (specification: Graph, Part 1).
//!
//! There is no compiler yet, so a person describes a graph in Rust with a
//! [`Builder`]. What it produces, a [`GraphDescription`], is plain data: it
//! names implementations and scalars, refers to nodes and inputs by
//! position, and could be written out unchanged. [`instantiate`] turns it
//! into a graph whose nodes hold handles.
//! Every name is resolved there, once, so that nothing is looked up by name
//! once it has returned (`docs/explorations/0009-designing-for-speed.md`).

mod builder;
mod instantiate;
mod registry;

use std::collections::HashSet;

use hgl_store::BindError;
use hgl_types::{NodeType, ScalarValue, TsType};

pub use builder::{Builder, NodeRef};
pub use instantiate::instantiate;
pub use registry::{Buildable, Ports, Registry};

/// A graph as wiring leaves it: which nodes, in what order, bound how.
/// Plain data, no closures, no pointers (GRF-1).
#[derive(Debug, Clone, PartialEq)]
pub struct GraphDescription {
    /// A name, for diagnostics.
    pub label: String,
    /// In rank order (GRF-3): the node at position `i` is `NodeId(i)` in
    /// every graph instantiated from this.
    pub nodes: Vec<NodeDescription>,
    /// Each binds one input to one output.
    pub edges: Vec<Edge>,
}

/// One node of a description.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeDescription {
    /// The name an implementation is registered under (GRF-9).
    pub implementation: String,
    /// A name, for diagnostics and error reports.
    pub label: String,
    /// One value for each scalar the node type declares (GRF-8).
    pub scalars: Vec<(String, ScalarValue)>,
}

/// One input bound to the output of an earlier node (GRF-4). P1 binds whole
/// `TS` inputs to whole `TS` outputs, so there are no paths yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// The producer's position in the description.
    pub source_node: u32,
    /// The consumer's position in the description.
    pub target_node: u32,
    /// The input's position in the consumer's node type.
    pub target_input: u32,
}

/// Why a graph could not be described or built.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// No implementation is registered under this name (GRF-9).
    UnknownImplementation(String),
    /// An implementation is already registered under this name.
    DuplicateImplementation(&'static str),
    /// The node type has no input by this name, or at this position.
    UnknownInput {
        /// The node's label; its position if no such node exists.
        node: String,
        /// The input's name, or its position.
        input: String,
    },
    /// The node type declares no output, or the node's build has already
    /// taken it.
    NoOutput {
        /// The node's label.
        node: String,
    },
    /// A scalar the node type does not declare, or one it declares that is
    /// not given exactly once (GRF-8).
    UnknownScalar {
        /// The node's label.
        node: String,
        /// The scalar's name.
        scalar: String,
    },
    /// An input, the output or a scalar is not of the type declared, or of
    /// the type asked for (GRF-7, GRF-8).
    WrongType {
        /// The node's label.
        node: String,
        /// The name of the input or scalar, or `output`.
        what: String,
    },
    /// A second edge to one input, or a node's build taking one input twice
    /// (GRF-6).
    InputBoundTwice {
        /// The node's label.
        node: String,
        /// The input's name.
        input: String,
    },
    /// No order puts every producer before its consumers; in a description,
    /// an edge that does not run forward (GRF-4).
    Cycle,
    /// The store refused a binding.
    Bind(BindError),
    /// Refused by `register`: an active or valid position past the last
    /// input, or two inputs with one name.
    InvalidNodeType {
        /// The node type's name.
        node: &'static str,
        /// What is wrong with its node type.
        what: String,
    },
}

/// From borrowed names, so that each check returns its error in one line.
impl BuildError {
    fn unknown_input(node: &str, input: &str) -> Self {
        Self::UnknownInput {
            node: node.to_owned(),
            input: input.to_owned(),
        }
    }

    fn no_output(node: &str) -> Self {
        Self::NoOutput {
            node: node.to_owned(),
        }
    }

    fn unknown_scalar(node: &str, scalar: &str) -> Self {
        Self::UnknownScalar {
            node: node.to_owned(),
            scalar: scalar.to_owned(),
        }
    }

    fn wrong_type(node: &str, what: &str) -> Self {
        Self::WrongType {
            node: node.to_owned(),
            what: what.to_owned(),
        }
    }

    fn bound_twice(node: &str, input: &str) -> Self {
        Self::InputBoundTwice {
            node: node.to_owned(),
            input: input.to_owned(),
        }
    }
}

/// GRF-8: every scalar given is one the node type declares, and every scalar
/// it declares is given once, of its type.
fn check_scalars(node_type: &NodeType, node: &NodeDescription) -> Result<(), BuildError> {
    let label = &node.label;
    let declared = &node_type.scalars;
    let given = &node.scalars;
    for (name, _) in given {
        if !declared.iter().any(|&(scalar, _)| scalar == name) {
            return Err(BuildError::unknown_scalar(label, name));
        }
    }
    for &(scalar, scalar_type) in declared {
        let matches: Vec<_> = given.iter().filter(|(name, _)| name == scalar).collect();
        let [(_, value)] = matches.as_slice() else {
            // Not given, or given more than once.
            return Err(BuildError::unknown_scalar(label, scalar));
        };
        if value.scalar_type() != scalar_type {
            return Err(BuildError::wrong_type(label, scalar));
        }
    }
    Ok(())
}

/// GRF-6, GRF-7: the source has an output, the target has the input, their
/// types are the same, and no edge before this one binds that input.
/// `bound` holds the (node, input) of every edge before this one. Both nodes
/// must exist.
fn check_edge(
    nodes: &[NodeDescription],
    types: &[&NodeType],
    edge: Edge,
    bound: &mut HashSet<(u32, u32)>,
) -> Result<(), BuildError> {
    let source = &nodes[edge.source_node as usize].label;
    let target = &nodes[edge.target_node as usize].label;
    let Some(TsType::Ts(output)) = types[edge.source_node as usize].output else {
        return Err(BuildError::no_output(source));
    };
    let inputs = &types[edge.target_node as usize].inputs;
    let Some(&(input, TsType::Ts(input_type))) = inputs.get(edge.target_input as usize) else {
        let position = edge.target_input.to_string();
        return Err(BuildError::unknown_input(target, &position));
    };
    if input_type != output {
        return Err(BuildError::wrong_type(target, input));
    }
    if !bound.insert((edge.target_node, edge.target_input)) {
        return Err(BuildError::bound_twice(target, input));
    }
    Ok(())
}

/// A position as the `u32` that edges, node ids and store ids hold.
#[expect(
    clippy::cast_possible_truncation,
    reason = "positions are u32 by design, as in the store and the kernel; debug builds assert the count fits"
)]
fn index(position: usize) -> u32 {
    debug_assert!(u32::try_from(position).is_ok(), "over u32::MAX");
    position as u32
}
