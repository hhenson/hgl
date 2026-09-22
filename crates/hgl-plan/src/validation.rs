//! Validate loaded templates before creating any endpoint.
use crate::project;
use crate::{
    BuildError, Catalog, ChildDescription, GraphDescription, InputPort, OutputPort, check_edge,
    check_scalars,
};
use hgl_types::TsType;

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
    let node = description
        .nodes
        .get(port.node as usize)
        .ok_or_else(missing)?;
    let kind = registry
        .node_type(&node.implementation)
        .ok_or_else(missing)?;
    let (_, input) = kind.inputs.get(port.input as usize).ok_or_else(missing)?;
    project(input, &port.path, false)
}

fn validate_child(
    child: &ChildDescription,
    owner: &hgl_types::NodeType,
    registry: &dyn Catalog,
) -> Result<(), BuildError> {
    let sources: Vec<_> = owner.inputs.iter().map(|(_, kind)| kind.clone()).collect();
    check_boundaries(child, &sources, registry)?;
    if let Some(output) = &child.output {
        let missing = || BuildError::no_output(&output.node.to_string());
        let output = output_type(output, &child.graph, registry)?;
        let expected = owner.output.as_ref().ok_or_else(missing)?;
        let expected = if let TsType::Reference(target) = expected {
            target.as_ref()
        } else {
            expected
        };
        let expected = if child.keyed {
            let TsType::Dictionary(target) = expected else {
                return Err(BuildError::InvalidChildren("keyed output".into()));
            };
            target.as_ref()
        } else {
            expected
        };
        if output != expected {
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
        if source != target && !matches!(source, TsType::Reference(t) if t.as_ref()==target) {
            return Err(BuildError::InvalidChildren("boundary shape".into()));
        }
        for earlier in &bound {
            if earlier.node == edge.target.node
                && earlier.input == edge.target.input
                && (earlier.path.starts_with(&edge.target.path)
                    || edge.target.path.starts_with(&earlier.path))
            {
                return Err(BuildError::bound_twice(
                    &edge.target.node.to_string(),
                    &edge.target.input.to_string(),
                ));
            }
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
    let node = description
        .nodes
        .get(port.node as usize)
        .ok_or_else(missing)?;
    let kind = registry
        .node_type(&node.implementation)
        .ok_or_else(missing)?;
    project(kind.output.as_ref().ok_or_else(missing)?, &port.path, false)
}
