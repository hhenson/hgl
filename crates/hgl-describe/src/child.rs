//! Child boundaries retain subscriptions instead of freezing current targets.
use crate::instantiate::{Quiet, walk_input};
use crate::{BuildError, BuiltGraph, ChildDescription, Registry, instantiate_complete};
use hgl_store::{InputId, Reference, Store};
use hgl_types::EngineTime;

/// Construct a template in the caller's child scope and bind before start.
/// The owning node supplies its input handles and, for keyed templates, the key.
pub fn instantiate_child(
    template: &ChildDescription,
    registry: &Registry,
    store: &mut Store,
    owners: &[InputId],
    key: Option<i64>,
    now: EngineTime,
) -> Result<BuiltGraph, BuildError> {
    if template.keyed && key.is_none() {
        return Err(BuildError::MissingKey);
    }
    let kinds: Vec<_> = owners
        .iter()
        .map(|&i| store.bindings().input(i).kind.clone())
        .collect();
    crate::plan::validate(&template.graph, registry)?;
    crate::plan::check_boundaries(template, &kinds, registry)?;
    let mut sources = Vec::new();
    for edge in &template.inputs {
        let mut source = owners[edge.source_input];
        for step in &edge.source_path {
            if store.bindings().input(source).reference_source.is_some() {
                return Err(BuildError::InvalidPath(
                    "project inside the child after capturing the whole REF target".into(),
                ));
            }
            source = walk_input(store, source, std::slice::from_ref(step), key)?;
        }
        sources.push(source);
    }
    let built = instantiate_complete(&template.graph, registry, store)?;
    for (edge, source) in template.inputs.iter().zip(sources) {
        copy_binding(store, built.input(&edge.target, store)?, source, now)?;
    }
    for &input in built.inputs.iter().flatten() {
        assemble(store, input, now)?;
    }
    Ok(built)
}

fn copy_binding(
    store: &mut Store,
    target: InputId,
    source: InputId,
    now: EngineTime,
) -> Result<(), BuildError> {
    let input = store.bindings().input(source);
    if let Some(reference) = input.reference_source {
        return store
            .follow(target, reference, now, &mut Quiet)
            .map_err(BuildError::Bind);
    }
    if let Some(output) = input.source {
        return if input.kind == store.bindings().input(target).kind {
            store.sample(target, store.reference(output), now, &mut Quiet)
        } else {
            store.follow(target, output, now, &mut Quiet)
        }
        .map_err(BuildError::Bind);
    }
    let count = input.fixed.len();
    for n in 0..count {
        copy_binding(
            store,
            store.bindings().fixed_input(target, n),
            store.bindings().fixed_input(source, n),
            now,
        )?;
    }
    assemble(store, target, now)?;
    Ok(())
}

/// Intern a newly wired assembly once; later ticks use its dense children.
pub(crate) fn assemble(
    store: &mut Store,
    input: InputId,
    now: EngineTime,
) -> Result<Reference, BuildError> {
    let i = store.bindings().input(input);
    if i.source.is_some() || !i.kind.fixed() {
        return Ok(store.bindings().input_reference(input));
    }
    let kind = i.kind.clone();
    let mut children = Vec::new();
    for n in 0..kind.len() {
        children.push(assemble(
            store,
            store.bindings().fixed_input(input, n),
            now,
        )?);
    }
    let reference = store
        .items_reference(kind, children)
        .map_err(BuildError::Bind)?;
    store
        .sample(input, reference, now, &mut Quiet)
        .map_err(BuildError::Bind)?;
    Ok(reference)
}
