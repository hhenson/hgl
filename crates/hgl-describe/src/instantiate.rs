//! A checked description brought to life in the caller's current scope.
use crate::registry::Ports;
use crate::{BuildError, GraphDescription, InputPort, OutputPort, Registry, Step, index};
use hgl_kernel::{Graph, NodeSlot};
use hgl_plan::validate;
use hgl_store::{InputId, OutputId, Store, Wake};
use hgl_types::{EngineTime, NodeId, NodeType, TsType};

pub(crate) struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}

/// Constructed graph plus its boundary handles. No instance handle enters its template.
#[derive(Debug)]
pub struct BuiltGraph {
    /// Independently owned graph instance.
    pub graph: Graph,
    /// Inputs by graph-local node and declared input position.
    pub inputs: Vec<Vec<InputId>>,
    /// Each node's optional output.
    pub outputs: Vec<Option<OutputId>>,
}
impl BuiltGraph {
    /// Resolve a checked fixed input path during construction.
    pub fn input(&self, port: &InputPort, store: &Store) -> Result<InputId, BuildError> {
        let id = self
            .inputs
            .get(port.node as usize)
            .and_then(|v| v.get(port.input as usize))
            .ok_or_else(|| {
                BuildError::unknown_input(&port.node.to_string(), &port.input.to_string())
            })?;
        walk_input(store, *id, &port.path, None)
    }
    /// Resolve a checked fixed output path during construction.
    pub fn output(&self, port: &OutputPort, store: &Store) -> Result<OutputId, BuildError> {
        let mut id = self
            .outputs
            .get(port.node as usize)
            .copied()
            .flatten()
            .ok_or_else(|| BuildError::no_output(&port.node.to_string()))?;
        for step in &port.path {
            let kind = &store.bindings().output(id).kind;
            let n = position(kind, step)?;
            id = store.bindings().fixed_output(id, n);
        }
        Ok(id)
    }
}

/// Instantiate a complete graph in the caller's current scope.
/// Bad descriptions add no endpoints; failed implementation constructors may
/// leave unbound storage. Child factories reclaim it through `Ctx::create_child`.
pub fn instantiate(
    description: &GraphDescription,
    registry: &Registry,
    store: &mut Store,
) -> Result<Graph, BuildError> {
    Ok(instantiate_complete(description, registry, store)?.graph)
}

/// Instantiate and retain the resolved endpoint handles for boundary construction.
pub fn instantiate_complete(
    description: &GraphDescription,
    registry: &Registry,
    store: &mut Store,
) -> Result<BuiltGraph, BuildError> {
    validate(description, registry)?;
    preflight_globals(description, registry, store)?;
    let mut slots = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for (position, node) in description.nodes.iter().enumerate() {
        let implementation = registry.find(&node.implementation)?;
        let node_type = &implementation.node_type;
        let mut ports = Ports {
            store,
            registry,
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
    let built = BuiltGraph {
        graph: Graph::new(description.label.clone(), slots),
        inputs,
        outputs,
    };
    for edge in &description.edges {
        let output = built.output(&edge.source, store)?;
        let input = built.input(&edge.target, store)?;
        if store.bindings().input(input).kind == store.bindings().output(output).kind {
            store.bind(input, output).map_err(BuildError::Bind)?;
        } else if matches!(&store.bindings().input(input).kind,TsType::Reference(child) if child.as_ref()==&store.bindings().output(output).kind)
        {
            store
                .bind_designation(input, output)
                .map_err(BuildError::Bind)?;
        } else {
            store
                .follow(input, output, EngineTime::NEVER, &mut Quiet)
                .map_err(BuildError::Bind)?;
        }
    }
    for inputs in &built.inputs {
        for &input in inputs {
            crate::child::assemble(store, input, EngineTime::NEVER)?;
        }
    }
    Ok(built)
}

pub(crate) fn walk_input(
    store: &Store,
    mut id: InputId,
    path: &[Step],
    key: Option<i64>,
) -> Result<InputId, BuildError> {
    if key.is_none() && path.contains(&Step::Key) {
        return Err(BuildError::MissingKey);
    }
    for step in path {
        hgl_plan::project(
            &store.bindings().input(id).kind,
            std::slice::from_ref(step),
            key.is_some(),
        )?;
        id = if *step == Step::Key {
            store
                .bindings()
                .child_input(id, key.ok_or(BuildError::MissingKey)?)
                .ok_or_else(|| BuildError::InvalidPath("absent member".into()))?
        } else {
            store
                .bindings()
                .fixed_input(id, position(&store.bindings().input(id).kind, step)?)
        };
    }
    Ok(id)
}
fn position(kind: &TsType, step: &Step) -> Result<usize, BuildError> {
    hgl_plan::project(kind, std::slice::from_ref(step), false)?;
    match step {
        Step::Field(name) => kind
            .field(name)
            .ok_or_else(|| BuildError::InvalidPath(name.clone())),
        Step::Index(n) => Ok(*n),
        Step::Key => Err(BuildError::InvalidPath("key on fixed path".into())),
    }
}
fn required(node_type: &NodeType, inputs: &[InputId]) -> Vec<InputId> {
    match &node_type.valid_inputs {
        None => inputs.to_vec(),
        Some(valid) => valid.iter().map(|&position| inputs[position]).collect(),
    }
}

fn preflight_globals(
    description: &GraphDescription,
    registry: &Registry,
    store: &mut Store,
) -> Result<(), BuildError> {
    for node in &description.nodes {
        let ty = &registry.find(&node.implementation)?.node_type;
        let invalid = |what| BuildError::InvalidNodeType {
            node: ty.name,
            what,
        };
        if ty.uses_global_state && !store.global_state_provisioned() {
            return Err(invalid("global_state: unprovisioned run store".into()));
        }
        for &(key, scalar) in &ty.global_entries {
            store
                .prepare_global(key, scalar)
                .map_err(|error| invalid(error.message))?;
        }
        for child in &node.children {
            preflight_globals(&child.graph, registry, store)?;
        }
    }
    Ok(())
}
