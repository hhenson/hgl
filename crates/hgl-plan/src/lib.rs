//! Plain, reusable graph templates and construction-time structural checks.
use hgl_store::BindError;
use hgl_types::{NodeType, ScalarValue};
mod builder;
mod paths;
pub use builder::{Builder, NodeRef};
mod validation;
pub use validation::{check_boundaries, input_type, output_type, validate};

/// Resolved implementation signatures, shared by builders and description validation.
pub trait Catalog: std::fmt::Debug {
    /// Signature registered under this implementation identity.
    fn node_type(&self, implementation: &str) -> Option<&NodeType>;
}
pub use paths::{check_edge, check_shape, project};

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
    /// Reusable child templates owned by this node.
    pub children: Vec<ChildDescription>,
}

/// One step down a resolved endpoint shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// Named bundle field.
    Field(String),
    /// Fixed list position.
    Index(usize),
    /// Current key of a keyed child instance; never an ordinary edge path.
    Key,
}
/// A node's input or fixed descendant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InputPort {
    /// Consumer position.
    pub node: u32,
    /// Declared input position.
    pub input: u32,
    /// Descendant path.
    pub path: Vec<Step>,
}
/// A node's output or fixed descendant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPort {
    /// Producer position.
    pub node: u32,
    /// Descendant path.
    pub path: Vec<Step>,
}
/// A graph-local connection; no runtime handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// Producer endpoint.
    pub source: OutputPort,
    /// Consumer endpoint.
    pub target: InputPort,
}
/// One owner input projected into a child.
#[derive(Debug, Clone, PartialEq)]
pub struct Boundary {
    /// Position in the owner's declared inputs.
    pub source_input: usize,
    /// Fixed projections, and optionally the current dictionary key.
    pub source_path: Vec<Step>,
    /// Child-local destination.
    pub target: InputPort,
}
/// A reusable child graph and its owner boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildDescription {
    /// Inert graph template.
    pub graph: GraphDescription,
    /// Bindings established before the child starts.
    pub inputs: Vec<Boundary>,
    /// Child output exposed to the owner, absent for a sink.
    pub output: Option<OutputPort>,
    /// The output is one dictionary member and boundary keys use this instance's key.
    pub keyed: bool,
}

/// Why a graph could not be described or built.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// A path does not name an endpoint of its declared shape.
    InvalidPath(String),
    /// Child templates or their interfaces do not match the owner.
    InvalidChildren(String),
    /// A keyed boundary was instantiated without its dictionary key.
    MissingKey,
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
    /// Construct a diagnostic from borrowed names.
    pub fn unknown_input(node: &str, input: &str) -> Self {
        Self::UnknownInput {
            node: node.to_owned(),
            input: input.to_owned(),
        }
    }

    /// Construct a diagnostic from borrowed names.
    pub fn no_output(node: &str) -> Self {
        Self::NoOutput {
            node: node.to_owned(),
        }
    }

    /// Construct a diagnostic from borrowed names.
    pub fn unknown_scalar(node: &str, scalar: &str) -> Self {
        Self::UnknownScalar {
            node: node.to_owned(),
            scalar: scalar.to_owned(),
        }
    }

    /// Construct a diagnostic from borrowed names.
    pub fn wrong_type(node: &str, what: &str) -> Self {
        Self::WrongType {
            node: node.to_owned(),
            what: what.to_owned(),
        }
    }

    /// Construct a diagnostic from borrowed names.
    pub fn bound_twice(node: &str, input: &str) -> Self {
        Self::InputBoundTwice {
            node: node.to_owned(),
            input: input.to_owned(),
        }
    }
}

/// GRF-8: every scalar given is one the node type declares, and every scalar
/// it declares is given once, of its type.
pub fn check_scalars(node_type: &NodeType, node: &NodeDescription) -> Result<(), BuildError> {
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

/// A position as the `u32` that edges, node ids and store ids hold.
pub fn index(position: usize) -> u32 {
    u32::try_from(position).unwrap_or_else(|_| unreachable!("graph capacity exceeded"))
}
