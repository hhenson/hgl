//! Symbolic generic occurrence requirements, intersected before specialization.
use hgl_library::{Decl, Library};
use hgl_source::{Ty, application, delta_argument};
use std::collections::BTreeSet;

/// Check every declared occurrence against complete canonical source arguments.
pub fn validate(library: &Library, declaration: &Decl, arguments: &[Ty]) -> Result<(), String> {
    let requirements = requirements(library, declaration, &mut BTreeSet::new())?;
    for (argument, requirement) in arguments.iter().zip(requirements) {
        if requirement & 1 != 0 && !hgl_value_access::ordinary(argument) {
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
    active: &mut BTreeSet<String>,
) -> Result<Vec<u8>, String> {
    let identity = format!("{}::{}", decl.module, decl.name);
    if !active.insert(identity.clone()) {
        return Err("recursive ordinary structs are not supported".into());
    }
    let schema = decl.required_struct()?;
    let mut result = vec![0; schema.generics.len()];
    for (_, field) in schema.fields {
        occurrences(
            library,
            &decl.module,
            &field,
            1,
            (&schema.generics, &mut result),
            active,
        )?;
    }
    active.remove(&identity);
    for requirement in &mut result {
        if *requirement == 0 {
            *requirement = 1;
        }
    }
    Ok(result)
}
fn occurrences(
    library: &Library,
    module: &str,
    pattern: &str,
    requirement: u8,
    (parameters, result): (&[String], &mut [u8]),
    active: &mut BTreeSet<String>,
) -> Result<(), String> {
    if let Some(index) = parameters.iter().position(|parameter| parameter == pattern) {
        result[index] |= requirement;
        return Ok(());
    }
    if let Some(origin) = delta_argument(pattern) {
        return occurrences(library, module, origin, 2, (parameters, result), active);
    }
    let Some((base, arguments)) = application(pattern) else {
        return Ok(());
    };
    if matches!(base, "list" | "tuple" | "map" | "set" | "atomic") {
        let requirement = if base == "atomic" { 1 } else { requirement };
        for argument in arguments
            .iter()
            .take(if base == "list" { 1 } else { arguments.len() })
        {
            occurrences(
                library,
                module,
                argument,
                requirement,
                (parameters, result),
                active,
            )?;
        }
        return Ok(());
    }
    if matches!(base, "ref" | "rolling") {
        return Err("ordinary struct fields require ordinary value types".into());
    }
    let decl = hgl_struct_names::declaration(library, module, base)?
        .ok_or_else(|| format!("unresolved generic struct application {base}"))?;
    let forwarded = requirements(library, decl, active)?;
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
            active,
        )?;
    }
    Ok(())
}
