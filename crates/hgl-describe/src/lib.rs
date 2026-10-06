//! Resolve inert graph descriptions into scoped runtime instances.
mod child;
pub use child::instantiate_child;
mod instantiate;
pub mod plan;
mod registry;

pub use instantiate::{BuiltGraph, instantiate, instantiate_complete};
use plan::index;
pub use plan::{
    Boundary, BuildError, ChildDescription, Edge, GraphDescription, InputPort, NodeDescription,
    OutputPort, Step,
};
pub use plan::{Builder, NodeRef};
pub use registry::{Buildable, Ports, Registry};
