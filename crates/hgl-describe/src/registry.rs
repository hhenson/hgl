//! What a node supplies so that it can be built, what it is given while it is
//! built, and where implementations are found by name.

use std::{collections::HashMap, sync::Arc};

use hgl_kernel::Node;
use hgl_store::{Global, GlobalValue, In, InputId, Out, OutputId, Scalar, Store};
use hgl_types::{NodeId, NodeType, TsType};

use crate::{BuildError, ChildDescription, NodeDescription};

/// The second half of a node, after its [`Node`] impl: its node type, and
/// how it is made from its ports.
///
/// ```
/// use hgl_describe::{Buildable, BuildError, Ports};
/// use hgl_kernel::{Ctx, Node, NodeResult};
/// use hgl_store::{In, Out};
/// use hgl_types::{NodeType, ScalarType, TsType};
///
/// struct Sum {
///     lhs: In<i64>,
///     rhs: In<i64>,
///     out: Out<i64>,
/// }
///
/// impl Node for Sum {
///     fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
///         ctx.set(self.out, ctx.get(self.lhs) + ctx.get(self.rhs));
///         Ok(())
///     }
/// }
///
/// impl Buildable for Sum {
///     fn node_type() -> NodeType {
///         let i64s = TsType::Ts(ScalarType::I64);
///         NodeType {
///             name: "sum",
///             inputs: vec![("lhs", i64s.clone()), ("rhs", i64s.clone())],
///             output: Some(i64s),
///             scalars: Vec::new(),
///             active_inputs: None,
///             valid_inputs: None,
///             uses_scheduler: false,
///             uses_global_state: false,
///             global_entries: Vec::new(),
///             schedule_on_start: false,
///             child_graphs: 0,
///         }
///     }
///     fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
///         Ok(Self { lhs: ports.input("lhs")?, rhs: ports.input("rhs")?, out: ports.output()? })
///     }
/// }
/// ```
pub trait Buildable: Node + Sized {
    /// What the runtime must know to make and run the node. Its name is the
    /// one the node is registered under, and that a description names it by
    /// (GRF-9).
    fn node_type() -> NodeType;
    /// Take the node's handles and scalars, by name: the only time they are
    /// looked up.
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError>;
}

/// A node's typed handles and scalars, by name, while it is being built.
///
/// Asking for an input or the output makes it in the store, owned by this
/// node. Whatever the build does not ask for is made when it returns, so the
/// store holds every port the node type declares whether the node reads it
/// or not.
#[derive(Debug)]
pub struct Ports<'a> {
    pub(crate) store: &'a mut Store,
    pub(crate) registry: &'a Registry,
    /// The node being built: the owner of every port made for it.
    pub(crate) node: NodeId,
    pub(crate) node_type: &'a NodeType,
    pub(crate) description: &'a NodeDescription,
    /// One entry per input of the node type, filled when the input is made.
    pub(crate) inputs: Vec<Option<InputId>>,
    pub(crate) output: Option<OutputId>,
}

impl Ports<'_> {
    /// Bind a declared global entry once, before this node can run a hook.
    pub fn global<T: GlobalValue>(&mut self, key: &str) -> Result<Global<T>, BuildError> {
        let invalid = |what| BuildError::InvalidNodeType {
            node: self.node_type.name,
            what,
        };
        if !self.node_type.uses_global_state
            || !self.node_type.global_entries.contains(&(key, T::schema()))
        {
            return Err(invalid(format!(
                "global_state: undeclared key/type {key:?}"
            )));
        }
        self.store
            .global_state()
            .bind(key)
            .map_err(|error| invalid(error.message))
    }
    /// The input called `name`, active or passive as the node type says.
    ///
    /// Errors: the node type has no such input, or declares another type
    /// for it; the input was taken already.
    pub fn input<T: Scalar>(&mut self, name: &str) -> Result<In<T>, BuildError> {
        let node_type = self.node_type;
        let label = &self.description.label;
        let inputs = &node_type.inputs;
        let Some(position) = inputs.iter().position(|&(input, _)| input == name) else {
            return Err(BuildError::unknown_input(label, name));
        };
        if inputs[position].1 != TsType::Ts(T::TYPE) {
            return Err(BuildError::wrong_type(label, name));
        }
        if self.inputs[position].is_some() {
            return Err(BuildError::bound_twice(label, name));
        }
        let input = self.store.add_input(self.node, active(node_type, position));
        self.inputs[position] = Some(input.id());
        Ok(input)
    }

    /// The node's output.
    ///
    /// Errors: the node type declares none, or declares another type; the
    /// output was taken already.
    pub fn output<T: Scalar>(&mut self) -> Result<Out<T>, BuildError> {
        let label = &self.description.label;
        let (None, Some(TsType::Ts(declared))) = (self.output, &self.node_type.output) else {
            return Err(BuildError::no_output(label));
        };
        if *declared != T::TYPE {
            return Err(BuildError::wrong_type(label, "output"));
        }
        let output = self.store.add_output(self.node);
        self.output = Some(output.id());
        Ok(output)
    }

    /// Allocate a recursively shaped input declared by this implementation.
    pub fn shaped_input(&mut self, name: &str) -> Result<InputId, BuildError> {
        let label = &self.description.label;
        let inputs = &self.node_type.inputs;
        let Some(n) = inputs.iter().position(|(field, _)| *field == name) else {
            return Err(BuildError::unknown_input(label, name));
        };
        if self.inputs[n].is_some() {
            return Err(BuildError::bound_twice(label, name));
        }
        let id = self.store.add_shaped_input(
            self.node,
            self.node_type.inputs[n].1.clone(),
            active(self.node_type, n),
        );
        self.inputs[n] = Some(id);
        Ok(id)
    }
    /// Allocate the recursively shaped output declared by this implementation.
    pub fn shaped_output(&mut self) -> Result<OutputId, BuildError> {
        let label = &self.description.label;
        let kind = self
            .node_type
            .output
            .as_ref()
            .ok_or_else(|| BuildError::no_output(label))?;
        if self.output.is_some() {
            return Err(BuildError::no_output(label));
        }
        let id = self.store.add_shaped_output(self.node, kind.clone());
        self.output = Some(id);
        Ok(id)
    }
    /// Finite key domains retained before node hooks can execute.
    pub fn keys(&mut self) -> &mut hgl_store::Keys {
        &mut self.store.keys
    }
    /// Shared projection and typed-handle reads during construction.
    pub fn store(&self) -> &Store {
        self.store
    }
    /// Implementations available to retained child templates.
    pub fn registry(&self) -> &Registry {
        self.registry
    }
    /// Reusable child templates; constructing the owner does not start them.
    pub fn children(&self) -> &[ChildDescription] {
        &self.description.children
    }

    /// The scalar called `name`, as the description gives it.
    ///
    /// Errors: the description gives no such scalar, or gives another type.
    pub fn scalar<T: Scalar>(&self, name: &str) -> Result<T, BuildError> {
        let scalars = &self.description.scalars;
        let label = &self.description.label;
        let Some((_, value)) = scalars.iter().find(|(scalar, _)| scalar == name) else {
            return Err(BuildError::unknown_scalar(label, name));
        };
        T::from_value(value.clone()).ok_or_else(|| BuildError::wrong_type(label, name))
    }

    /// Every port the node type declares, in its order, making each one the
    /// build did not ask for.
    pub(crate) fn into_ids(self) -> (Vec<InputId>, Option<OutputId>) {
        let mut inputs = Vec::with_capacity(self.inputs.len());
        for (position, (_, declared)) in self.node_type.inputs.iter().enumerate() {
            let active = active(self.node_type, position);
            inputs.push(self.inputs[position].unwrap_or_else(|| {
                self.store
                    .add_shaped_input(self.node, declared.clone(), active)
            }));
        }
        let output = self.output.or_else(|| {
            self.node_type
                .output
                .as_ref()
                .map(|declared| self.store.add_shaped_output(self.node, declared.clone()))
        });
        (inputs, output)
    }
}

/// Whether the input at `position` wakes its node when notified (NOD-4,
/// GRF-17).
fn active(node_type: &NodeType, position: usize) -> bool {
    node_type
        .active_inputs
        .as_ref()
        .is_none_or(|active| active.contains(&position))
}

/// What the registry keeps for one implementation.
#[derive(Clone)]
pub(crate) struct Implementation {
    pub(crate) node_type: NodeType,
    pub(crate) build: Arc<Factory>,
}

type Factory = dyn Fn(&mut Ports<'_>) -> Result<Box<dyn Node>, BuildError> + Send + Sync;
impl std::fmt::Debug for Implementation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Implementation")
            .field("node_type", &self.node_type)
            .finish_non_exhaustive()
    }
}

/// Every implementation a description may name, by the name it is
/// registered under.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    implementations: HashMap<&'static str, Implementation>,
}

impl Registry {
    /// A registry holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Make `N` available under its node type's name. Errors if that name is
    /// taken, or the node type is not well formed; either way what is
    /// registered stays as it was.
    pub fn register<N: Buildable>(&mut self) -> Result<(), BuildError> {
        self.register_with(N::node_type(), N::build)
    }

    /// Register a typed construction factory retaining prepared configuration.
    pub fn register_with<N: Node + 'static>(
        &mut self,
        node_type: NodeType,
        constructor: impl Fn(&mut Ports<'_>) -> Result<N, BuildError> + Send + Sync + 'static,
    ) -> Result<(), BuildError> {
        let name = node_type.name;
        if self.implementations.contains_key(name) {
            return Err(BuildError::DuplicateImplementation(name));
        }
        for kind in node_type
            .inputs
            .iter()
            .map(|(_, kind)| kind)
            .chain(node_type.output.iter())
        {
            hgl_plan::check_shape(kind)?;
        }
        node_type
            .validate_metadata()
            .map_err(|what| BuildError::InvalidNodeType { node: name, what })?;
        let implementation = Implementation {
            node_type,
            build: Arc::new(move |ports| Ok(Box::new(constructor(ports)?))),
        };
        self.implementations.insert(name, implementation);
        Ok(())
    }

    /// The node type of the implementation registered as `implementation`.
    pub fn node_type(&self, implementation: &str) -> Option<&NodeType> {
        let found = self.implementations.get(implementation)?;
        Some(&found.node_type)
    }

    /// The implementation registered as `implementation` (GRF-9).
    pub(crate) fn find(&self, implementation: &str) -> Result<&Implementation, BuildError> {
        self.implementations
            .get(implementation)
            .ok_or_else(|| BuildError::UnknownImplementation(implementation.to_owned()))
    }
}

impl hgl_plan::Catalog for Registry {
    fn node_type(&self, implementation: &str) -> Option<&NodeType> {
        Registry::node_type(self, implementation)
    }
}
