//! Scoped inherited field checking and closed declaration-family formation.
use hgl_inheritance::Pattern;
use hgl_library::{Decl, Library, Role};
use hgl_source::{FamilyType, Nominal, Ty};
use hgl_struct_names::declaration;
use std::collections::{BTreeMap, BTreeSet};
/// Resolve an inherited field without changing declaration-owned name scopes.
pub fn field_type(
    pattern: &Pattern,
    bindings: &BTreeMap<String, Ty>,
    mut resolve: impl FnMut(&str, &str, &BTreeMap<String, Ty>) -> Result<Ty, String>,
) -> Result<Ty, String> {
    pattern.resolve(bindings, &mut |module, name, args| {
        let (name, bindings) = application_bindings(name, args);
        resolve(module, &name, &bindings)
    })
}
/// Infer root type parameters through a scoped inherited field pattern.
pub fn infer_field(
    pattern: &Pattern,
    actual: &Ty,
    bindings: &mut BTreeMap<String, Ty>,
    mut unify: impl FnMut(&str, &str, &Ty, &[String], &mut BTreeMap<String, Ty>) -> Result<(), String>,
) -> Result<(), String> {
    pattern.infer(actual, bindings, &mut |module, name, count, actual| {
        if name == "atomic" && count == 1 {
            return Ok(vec![hgl_value_access::project(actual)]);
        }
        let parameters = (0..count)
            .map(|i| format!("Inherited{i}"))
            .collect::<Vec<_>>();
        let spelling = spelling(name, &parameters);
        let mut inferred = BTreeMap::new();
        unify(module, &spelling, actual, &parameters, &mut inferred)?;
        parameters
            .iter()
            .map(|p| {
                inferred
                    .remove(p)
                    .ok_or_else(|| "unresolved inherited argument".into())
            })
            .collect()
    })
}
fn application_bindings(name: &str, args: &[Ty]) -> (String, BTreeMap<String, Ty>) {
    let bindings = args
        .iter()
        .enumerate()
        .map(|(i, ty)| (format!("Inherited{i}"), ty.clone()))
        .collect::<BTreeMap<_, _>>();
    let parameters = (0..args.len())
        .map(|i| format!("Inherited{i}"))
        .collect::<Vec<_>>();
    let spelling = spelling(name, &parameters);
    (spelling, bindings)
}
fn spelling(name: &str, args: &[String]) -> String {
    if let Some(size) = name.strip_prefix("list<").and_then(|s| s.strip_suffix('>')) {
        return format!("list<{},{}>", args[0], size);
    }
    if args.is_empty() {
        name.into()
    } else {
        format!("{name}<{}>", args.join(","))
    }
}
/// Freeze every declared descendant compatible with one exact family specialization.
pub fn resolve(
    library: &Library,
    root: &Decl,
    args: &[Ty],
    active: &mut BTreeSet<String>,
    mut specialize: impl FnMut(&Decl, Vec<Ty>, &mut BTreeSet<String>) -> Result<Ty, String>,
    mut resolve: impl FnMut(&str, &str, &BTreeMap<String, Ty>) -> Result<Ty, String>,
) -> Result<Ty, String> {
    let identity = Nominal {
        origin: format!("{}::{}", root.module, root.name),
        arguments: args.to_vec(),
    };
    if !active.insert(identity.source_name()) {
        return Err("recursive abstract family fields are unsupported".into());
    }
    let root_bindings = root
        .required_struct()?
        .generics
        .into_iter()
        .zip(args.iter().cloned())
        .collect();
    let ancestors = hgl_inheritance::ancestors(library, root)?
        .iter()
        .map(|p| nominal(library, p, &root_bindings, &mut resolve))
        .collect::<Result<Vec<_>, _>>()?;
    let mut members = Vec::new();
    for candidate in library
        .declarations
        .iter()
        .filter(|d| d.role == Role::Struct)
    {
        let schema = candidate.required_struct()?;
        if schema.abstract_type {
            continue;
        }
        for ancestor in hgl_inheritance::ancestors(library, candidate)? {
            let mut bindings = BTreeMap::new();
            if ancestor
                .infer(
                    &Ty::Struct(identity.clone(), Vec::new(), Vec::new()),
                    &mut bindings,
                    &mut |module, name, count, actual| {
                        match_nominal(library, module, name, count, actual)
                    },
                )
                .is_err()
            {
                continue;
            }
            let Some(arguments) = schema
                .generics
                .iter()
                .map(|p| bindings.get(p).cloned())
                .collect::<Option<Vec<_>>>()
            else {
                return Err("family member has unapplied invariant type arguments".into());
            };
            let member = specialize(candidate, arguments, active)?;
            let member_id = member.structure()?.0.clone();
            if !member.atomic_payload() {
                return Err("family member requires a finite complete ordinary payload".into());
            }
            members.push((member_id, hgl_value_access::project(&member)));
            break;
        }
    }
    active.remove(&identity.source_name());
    Ok(Ty::Family(FamilyType::new(identity, ancestors, members)?))
}
fn match_nominal(
    library: &Library,
    module: &str,
    name: &str,
    count: usize,
    actual: &Ty,
) -> Result<Vec<Ty>, String> {
    if let Some(size) = name.strip_prefix("list<").and_then(|s| s.strip_suffix('>'))
        && let Ty::List(child, Some(actual_size)) = actual
        && count == 1
        && size.parse::<usize>().ok() == Some(*actual_size)
    {
        return Ok(vec![*child.clone()]);
    }
    let children = match (name, actual) {
        ("list", Ty::List(child, None))
        | ("set", Ty::Set(child))
        | ("atomic", Ty::Atomic(child)) => Some(vec![*child.clone()]),
        ("tuple", Ty::Tuple(children)) => Some(children.clone()),
        ("map", Ty::Map(key, child)) => Some(vec![*key.clone(), *child.clone()]),
        _ => None,
    };
    if let Some(children) = children
        && children.len() == count
    {
        return Ok(children);
    }
    if let Ty::Struct(id, _, _) = actual {
        let decl = declaration(library, module, name)?.ok_or("unresolved ancestor")?;
        if id.origin == format!("{}::{}", decl.module, decl.name) && count == id.arguments.len() {
            return Ok(id.arguments.clone());
        }
    }
    if count == 0 && Ty::parse(name).as_ref() == Some(actual) {
        return Ok(Vec::new());
    }
    Err("different invariant ancestor specialization".into())
}
fn nominal(
    library: &Library,
    pattern: &Pattern,
    bindings: &BTreeMap<String, Ty>,
    resolve: &mut impl FnMut(&str, &str, &BTreeMap<String, Ty>) -> Result<Ty, String>,
) -> Result<Nominal, String> {
    let Pattern::Named(module, name, args) = pattern else {
        return Err("ancestor requires a nominal declaration".into());
    };
    let decl = declaration(library, module, name)?.ok_or("unresolved ancestor")?;
    let arguments = args
        .iter()
        .map(|arg| field_type(arg, bindings, &mut *resolve))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Nominal {
        origin: format!("{}::{}", decl.module, decl.name),
        arguments,
    })
}

mod source_argument;
pub use source_argument::source_argument;
