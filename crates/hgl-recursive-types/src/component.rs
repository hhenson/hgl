use hgl_library::{Decl, Library};
use hgl_source::{application, delta_argument};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn identity(decl: &Decl) -> String {
    format!("{}::{}", decl.module, decl.name)
}
fn references<'a>(
    library: &'a Library,
    decl: &Decl,
    name: &str,
    result: &mut Vec<&'a Decl>,
) -> Result<(), String> {
    if decl
        .required_struct()?
        .generics
        .iter()
        .any(|parameter| parameter == name)
    {
        return Ok(());
    }
    if let Some(origin) = delta_argument(name) {
        return references(library, decl, origin, result);
    }
    let (base, args) = application(name).unwrap_or((name, Vec::new()));
    if let Some(target) = hgl_struct_names::declaration(library, &decl.module, base)? {
        result.push(target);
    }
    for arg in args
        .iter()
        .take(if base == "list" { 1 } else { args.len() })
    {
        references(library, decl, arg, result)?;
    }
    Ok(())
}
fn reaches(
    graph: &BTreeMap<String, Vec<String>>,
    from: &str,
    to: &str,
    seen: &mut BTreeSet<String>,
) -> bool {
    if !seen.insert(from.into()) {
        return false;
    }
    graph.get(from).is_some_and(|children| {
        children
            .iter()
            .any(|child| child == to || reaches(graph, child, to, seen))
    })
}
pub(super) fn members<'a>(library: &'a Library, root: &'a Decl) -> Result<Vec<&'a Decl>, String> {
    let mut pending = vec![root];
    let mut declarations = BTreeMap::new();
    let mut graph = BTreeMap::new();
    while let Some(decl) = pending.pop() {
        let name = identity(decl);
        if declarations.insert(name.clone(), decl).is_some() {
            continue;
        }
        let mut children = Vec::new();
        for (_, field) in decl.required_struct()?.fields {
            references(library, decl, &field, &mut children)?;
        }
        graph.insert(name, children.iter().map(|decl| identity(decl)).collect());
        pending.extend(children);
    }
    let name = identity(root);
    if !reaches(&graph, &name, &name, &mut BTreeSet::new()) {
        return Ok(Vec::new());
    }
    let members = declarations
        .into_iter()
        .filter_map(|(id, decl)| reaches(&graph, &id, &name, &mut BTreeSet::new()).then_some(decl))
        .collect::<Vec<_>>();
    if members.iter().any(|decl| decl.module != root.module) {
        return Err("recursive cycles must stay within one module".into());
    }
    for decl in &members {
        for (_, field) in decl.required_struct()?.fields {
            edge(library, decl, &field, &members)?;
        }
    }
    Ok(members)
}
pub(super) fn edge<'a, 'b>(
    library: &'a Library,
    decl: &Decl,
    pattern: &'b str,
    members: &[&Decl],
) -> Result<Option<(&'a Decl, Vec<&'b str>)>, String> {
    let mut targets = Vec::new();
    references(library, decl, pattern, &mut targets)?;
    let recursive = |target: &Decl| {
        members
            .iter()
            .any(|decl| identity(decl) == identity(target))
    };
    if !targets.iter().any(|target| recursive(target)) {
        return Ok(None);
    }
    let Some(("atomic", outer)) = application(pattern) else {
        return Err("recursive edges must be direct atomic fields with a null default".into());
    };
    let [payload] = outer.as_slice() else {
        return Err("recursive atomic boundary takes one type".into());
    };
    let (base, args) = application(payload).unwrap_or((payload, Vec::new()));
    let target = hgl_struct_names::declaration(library, &decl.module, base)?
        .filter(|target| recursive(target))
        .ok_or("recursive edges through containers or generic arguments are unsupported")?;
    let parameters = decl.required_struct()?.generics;
    for argument in &args {
        if parameters
            .iter()
            .any(|parameter| contains_parameter(argument, parameter))
            && !parameters.iter().any(|parameter| parameter == argument)
        {
            return Err(
                "recursive specialization arguments must be unchanged parameters or closed types"
                    .into(),
            );
        }
        let mut nested = Vec::new();
        references(library, decl, argument, &mut nested)?;
        if nested.iter().any(|target| recursive(target)) {
            return Err("recursive edges through generic arguments are unsupported".into());
        }
    }
    Ok(Some((target, args)))
}
fn contains_parameter(pattern: &str, parameter: &str) -> bool {
    pattern == parameter
        || application(pattern)
            .is_some_and(|(_, args)| args.iter().any(|arg| contains_parameter(arg, parameter)))
}
