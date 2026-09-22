//! Resolve inert graph descriptions into scoped runtime instances.
mod child;
pub use child::instantiate_child;
mod instantiate;
mod registry;

use hgl_plan::index;
pub use hgl_plan::{
    Boundary, BuildError, ChildDescription, Edge, GraphDescription, InputPort, NodeDescription,
    OutputPort, Step,
};
pub use hgl_plan::{Builder, NodeRef};
pub use instantiate::{BuiltGraph, instantiate, instantiate_complete};
pub use registry::{Buildable, Ports, Registry};
