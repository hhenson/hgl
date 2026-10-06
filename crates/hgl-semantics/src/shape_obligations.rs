//! Symbolic generic occurrence requirements, intersected before specialization.
use crate::library::{Decl, Library};
use hgl_source::{Ty, application, delta_argument};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
struct Requirements {
    active: BTreeSet<String>,
    known: BTreeMap<String, Vec<u8>>,
}

/// Check every declared occurrence against complete canonical source arguments.
pub fn validate(library: &Library, declaration: &Decl, arguments: &[Ty]) -> Result<(), String> {
    let mut state = Requirements::default();
    let requirements = loop {
        let previous = state.known.clone();
        let result = requirements(library, declaration, &mut state)?;
        if previous == state.known {
            break result;
        }
    };
    for (argument, requirement) in arguments.iter().zip(requirements) {
        let requirement = if requirement == 0 { 1 } else { requirement };
        if requirement & 1 != 0 && !crate::value_access::ordinary(argument) {
            return Err(
                "struct type argument requires an ordinary value type at every ordinary occurrence"
                    .into(),
            );
        }
        if requirement & 2 != 0 && !argument.publication() {
            return Err("struct type argument has no admitted delta type".into());
        }
    }
    Ok(())
}
fn requirements(
    library: &Library,
    decl: &Decl,
    state: &mut Requirements,
) -> Result<Vec<u8>, String> {
    let identity = format!("{}::{}", decl.module, decl.name);
    let schema = decl.required_struct()?;
    if !state.active.insert(identity.clone()) {
        return Ok(state
            .known
            .get(&identity)
            .cloned()
            .unwrap_or_else(|| vec![0; schema.generics.len()]));
    }
    let mut result = vec![0; schema.generics.len()];
    for (_, field) in schema.fields {
        occurrences(
            library,
            &decl.module,
            &field,
            1,
            (&schema.generics, &mut result),
            state,
        )?;
    }
    state.active.remove(&identity);
    if let Some(previous) = state.known.get(&identity) {
        for (requirement, old) in result.iter_mut().zip(previous) {
            *requirement |= old;
        }
    }
    state.known.insert(identity, result.clone());
    Ok(result)
}
fn occurrences(
    library: &Library,
    module: &str,
    pattern: &str,
    requirement: u8,
    (parameters, result): (&[String], &mut [u8]),
    state: &mut Requirements,
) -> Result<(), String> {
    if let Some(index) = parameters.iter().position(|parameter| parameter == pattern) {
        result[index] |= requirement;
        return Ok(());
    }
    if let Some(origin) = delta_argument(pattern) {
        return occurrences(library, module, origin, 2, (parameters, result), state);
    }
    let Some((base, arguments)) = application(pattern) else {
        return Ok(());
    };
    if matches!(
        base,
        "list" | "tuple" | "map" | "set" | "atomic" | "rolling"
    ) {
        let requirement = if matches!(base, "atomic" | "rolling") {
            1
        } else {
            requirement
        };
        for argument in arguments
            .iter()
            .take(if matches!(base, "list" | "rolling") {
                1
            } else {
                arguments.len()
            })
        {
            occurrences(
                library,
                module,
                argument,
                requirement,
                (parameters, result),
                state,
            )?;
        }
        return Ok(());
    }
    if base == "ref" {
        return Err("ordinary struct fields require ordinary value types".into());
    }
    let decl = crate::struct_names::declaration(library, module, base)?
        .ok_or_else(|| format!("unresolved generic struct application {base}"))?;
    let forwarded = requirements(library, decl, state)?;
    if forwarded.len() != arguments.len() {
        return Err("generic struct argument count mismatch".into());
    }
    for (argument, requirement) in arguments.into_iter().zip(forwarded) {
        occurrences(
            library,
            module,
            argument,
            requirement,
            (parameters, result),
            state,
        )?;
    }
    Ok(())
}
