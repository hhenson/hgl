//! Structural checks shared by hand wiring and loaded descriptions.
use crate::{BuildError, Edge, InputPort, NodeDescription, Step};
use hgl_types::{NodeType, TsType};

/// Resolve a path through a declared shape, without touching storage.
pub fn project<'a>(kind: &'a TsType, path: &[Step], keyed: bool) -> Result<&'a TsType, BuildError> {
    path.iter().try_fold(kind, |kind, step| {
        match step {
            Step::Field(name) => kind.field(name).map(|n| kind.child(n)),
            Step::Index(n) => {
                (matches!(kind, TsType::List(..)) && *n < kind.len()).then(|| kind.child(*n))
            }
            Step::Key
                if keyed && matches!(kind, TsType::Dictionary(_) | TsType::KeyedDictionary(..)) =>
            {
                kind.member()
            }
            Step::Key => None,
        }
        .ok_or_else(|| BuildError::InvalidPath(format!("{step:?}")))
    })
}

/// Reject ambiguous field names anywhere in a recursive shape.
pub fn check_shape(kind: &TsType) -> Result<(), BuildError> {
    match kind {
        TsType::Ts(_) | TsType::Atomic(_) | TsType::Set(_) | TsType::KeyedSet(_) => Ok(()),
        TsType::Dictionary(child)
        | TsType::KeyedDictionary(_, child)
        | TsType::Reference(child)
        | TsType::List(child, _) => check_shape(child),
        TsType::Bundle(fields) => {
            for (n, (name, child)) in fields.iter().enumerate() {
                if fields[..n].iter().any(|(previous, _)| previous == name) {
                    return Err(BuildError::InvalidPath(format!("duplicate field {name}")));
                }
                check_shape(child)?;
            }
            Ok(())
        }
    }
}

/// GRF-6/7: a compatible edge whose target overlaps no previous target.
pub fn check_edge(
    nodes: &[NodeDescription],
    types: &[&NodeType],
    edge: &Edge,
    bound: &mut Vec<InputPort>,
) -> Result<(), BuildError> {
    let source = edge.source.node as usize;
    let target = edge.target.node as usize;
    let missing = || BuildError::unknown_input(&target.to_string(), &edge.target.input.to_string());
    let source_type = types.get(source).ok_or_else(missing)?;
    let target_type = types.get(target).ok_or_else(missing)?;
    let out = source_type
        .output
        .as_ref()
        .ok_or_else(|| BuildError::no_output(&nodes[source].label))?;
    let (name, input) = target_type
        .inputs
        .get(edge.target.input as usize)
        .ok_or_else(|| {
            BuildError::unknown_input(&nodes[target].label, &edge.target.input.to_string())
        })?;
    let out = project(out, &edge.source.path, false)?;
    let input = project(input, &edge.target.path, false)?;
    let follows = matches!(out, TsType::Reference(child) if child.as_ref() == input);
    let captures = matches!(input,TsType::Reference(child) if child.as_ref()==out);
    if input != out && !follows && !captures {
        return Err(BuildError::wrong_type(&nodes[target].label, name));
    }
    for previous in bound.iter() {
        if previous.node == edge.target.node
            && previous.input == edge.target.input
            && (previous.path.starts_with(&edge.target.path)
                || edge.target.path.starts_with(&previous.path))
        {
            return Err(BuildError::bound_twice(&nodes[target].label, name));
        }
    }
    bound.push(edge.target.clone());
    Ok(())
}
