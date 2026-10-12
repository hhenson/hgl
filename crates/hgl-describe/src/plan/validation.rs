//! Validate loaded templates before creating any endpoint.
use crate::plan::project;
use crate::plan::{
    BuildError, Catalog, ChildDescription, GraphDescription, InputPort, OutputPort, check_edge,
    check_scalars,
};
use hgl_types::{NodeType, TsType};

/// Check a complete recursive description without allocating runtime endpoints.
pub fn validate(description: &GraphDescription, registry: &dyn Catalog) -> Result<(), BuildError> {
    let mut types = Vec::new();
    for node in &description.nodes {
        let kind = registry
            .node_type(&node.implementation)
            .ok_or_else(|| BuildError::UnknownImplementation(node.implementation.clone()))?;
        check_scalars(kind, node)?;
        if node.children.len() != kind.child_graphs {
            return Err(BuildError::InvalidChildren(node.label.clone()));
        }
        types.push(kind);
    }
    let mut bound = Vec::new();
    for edge in &description.edges {
        if edge.source.node >= edge.target.node {
            return Err(BuildError::Cycle);
        }
        check_edge(&description.nodes, &types, edge, &mut bound)?;
    }
    for (node, kind) in description.nodes.iter().zip(types) {
        for child in &node.children {
            validate(&child.graph, registry)?;
            validate_child(child, kind, registry)?;
        }
    }
    Ok(())
}

/// Check a complete recursive description without allocating runtime endpoints.
pub fn input_type<'a>(
    port: &InputPort,
    description: &GraphDescription,
    registry: &'a dyn Catalog,
) -> Result<&'a TsType, BuildError> {
    let missing = || BuildError::unknown_input(&port.node.to_string(), &port.input.to_string());
    let kind = node_type(description, registry, port.node).ok_or_else(missing)?;
    let (_, input) = kind.inputs.get(port.input as usize).ok_or_else(missing)?;
    project(input, &port.path, false)
}

fn validate_child(
    child: &ChildDescription,
    owner: &NodeType,
    registry: &dyn Catalog,
) -> Result<(), BuildError> {
    let sources: Vec<_> = owner.inputs.iter().map(|(_, kind)| kind.clone()).collect();
    check_boundaries(child, &sources, registry)?;
    if let Some(output) = &child.output {
        let missing = || BuildError::no_output(&output.node.to_string());
        let output = output_type(output, &child.graph, registry)?;
        let mut expected = owner.output.as_ref().ok_or_else(missing)?;
        if let TsType::Reference(target) = expected {
            expected = target;
        }
        if child.keyed {
            let (TsType::Dictionary(target) | TsType::KeyedDictionary(_, target)) = expected else {
                return Err(BuildError::InvalidChildren("keyed output".into()));
            };
            expected = target;
        }
        let reference_export = !child.keyed
            && matches!(owner.output, Some(TsType::Reference(_)))
            && Some(output) == owner.output.as_ref();
        if output != expected && !reference_export {
            return Err(BuildError::InvalidChildren("child output shape".into()));
        }
    }
    Ok(())
}

/// Validate owner-input interfaces before a child instance allocates storage.
pub fn check_boundaries(
    child: &ChildDescription,
    sources: &[TsType],
    registry: &dyn Catalog,
) -> Result<(), BuildError> {
    let mut bound: Vec<_> = child.graph.edges.iter().map(|e| e.target.clone()).collect();
    for edge in &child.inputs {
        let source = sources
            .get(edge.source_input)
            .ok_or_else(|| BuildError::InvalidChildren("missing owner input".into()))?;
        let source = project(source, &edge.source_path, child.keyed)?;
        let target = input_type(&edge.target, &child.graph, registry)?;
        if source != target
            && !matches!(source, TsType::Reference(t) if t.as_ref()==target)
            && !matches!(target, TsType::Reference(t) if t.as_ref()==source)
        {
            return Err(BuildError::InvalidChildren("boundary shape".into()));
        }
        if bound.iter().any(|previous| previous.overlaps(&edge.target)) {
            return Err(BuildError::bound_twice(
                &edge.target.node.to_string(),
                &edge.target.input.to_string(),
            ));
        }
        bound.push(edge.target.clone());
    }
    if let Some(output) = &child.output {
        output_type(output, &child.graph, registry)?;
    }
    Ok(())
}
/// Resolve a declared output path, without an instance.
pub fn output_type<'a>(
    port: &OutputPort,
    description: &GraphDescription,
    registry: &'a dyn Catalog,
) -> Result<&'a TsType, BuildError> {
    let missing = || BuildError::no_output(&port.node.to_string());
    let kind = node_type(description, registry, port.node).ok_or_else(missing)?;
    project(kind.output.as_ref().ok_or_else(missing)?, &port.path, false)
}

fn node_type<'a>(
    description: &GraphDescription,
    registry: &'a dyn Catalog,
    node: u32,
) -> Option<&'a NodeType> {
    registry.node_type(&description.nodes.get(node as usize)?.implementation)
}
