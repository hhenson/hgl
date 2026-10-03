//! Resolve finite ordinary value schemas against source library declarations.
use hgl_library::{Decl, Library, Role};
use hgl_source::{Nominal, Ty, application, delta_argument};
use hgl_value_check::ordinary;
use std::collections::{BTreeMap, BTreeSet};
/// Resolve imported declaration identity.
pub fn identity(library: &Library, module: &str, name: &str) -> (String, String) {
    if let Some((alias, item)) = name.split_once("::") {
        return (
            library
                .imports
                .get(&(module.into(), alias.into()))
                .cloned()
                .unwrap_or_else(|| alias.into()),
            item.into(),
        );
    }
    if let Some(target) = library.imports.get(&(module.into(), name.into()))
        && let Some((owner, item)) = target.rsplit_once("::")
    {
        return (owner.into(), item.into());
    }
    (module.into(), name.into())
}

/// Find a visible required-field struct declaration.
pub fn declaration<'a>(
    library: &'a Library,
    module: &str,
    name: &str,
) -> Result<Option<&'a Decl>, String> {
    let local = !name.contains("::")
        && library
            .declarations
            .iter()
            .any(|d| d.module == module && d.name == name && d.role == Role::Struct);
    let (owner, item) = if local {
        (module.into(), name.into())
    } else {
        identity(library, module, name)
    };
    let mut found = library
        .declarations
        .iter()
        .filter(|d| d.module == owner && d.name == item && d.role == Role::Struct);
    let declaration = found.next();
    if found.next().is_some() {
        return Err(format!("duplicate struct {owner}::{item}"));
    }
    if let Some(decl) = declaration {
        if decl.module != module && !exported(decl) {
            return Err(format!(
                "{}::{}: struct is not exported",
                decl.module, decl.name
            ));
        }
        if let Some((alias, _)) = name.split_once("::")
            && !library.imports.contains_key(&(module.into(), alias.into()))
        {
            return Err(format!(
                "struct qualification requires an imported module alias {alias}"
            ));
        }
    }
    Ok(declaration)
}
/// Resolve a finite ordinary type, retaining nominal identity.
pub fn resolve(
    library: &Library,
    module: &str,
    name: &str,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    substitute(library, module, name, &BTreeMap::new(), active)
}
/// Substitute declared type parameters through finite ordinary source shapes.
pub fn substitute(
    library: &Library,
    module: &str,
    name: &str,
    bindings: &BTreeMap<String, Ty>,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    let normalized = hgl_type_sizes::normalize(name, &mut |expr| size(library, module, expr))?;
    let name = normalized.as_str();
    if let Some(ty) = bindings.get(name) {
        return Ok(ty.clone());
    }
    if let Some(origin) = delta_argument(name) {
        return substitute(library, module, origin, bindings, active)?.delta();
    }
    if let Some(ty) = Ty::parse(name) {
        return Ok(ty);
    }
    if let Some((element, size)) = Ty::list_parts(name) {
        return Ok(Ty::List(
            Box::new(substitute(library, module, element, bindings, active)?),
            size,
        ));
    }
    let (base, arguments) =
        application(name).map_or((name, Vec::new()), |(base, args)| (base, args));
    if matches!(base, "map" | "tuple" | "set") {
        let children = arguments
            .into_iter()
            .map(|arg| substitute(library, module, arg, bindings, active))
            .collect::<Result<Vec<_>, _>>()?;
        return match (base, children.as_slice()) {
            ("map", [key, child]) => Ok(Ty::Map(Box::new(key.clone()), Box::new(child.clone()))),
            ("set", [member]) => Ok(Ty::Set(Box::new(member.clone()))),
            ("tuple", _) => Ok(Ty::Tuple(children)),
            _ => Err(format!("invalid structural type arguments {name}")),
        };
    }
    if matches!(base, "atomic" | "rolling" | "ref") {
        return Err(format!(
            "unsupported ordinary struct argument or field {name}"
        ));
    }
    let decl = declaration(library, module, base)?
        .ok_or_else(|| format!("unresolved ordinary type {name}"))?;
    let arguments = arguments.into_iter().map(|arg| {
        if arg == "_" { return Err("struct arguments require complete concrete types; placeholders are not admitted".into()); }
        substitute(library, module, arg, bindings, active)
    }).collect::<Result<Vec<_>, String>>()?;
    specialize(library, decl, arguments, active)
}
/// Build a validated nominal specialization with fully substituted fields.
pub fn specialize(
    library: &Library,
    decl: &Decl,
    arguments: Vec<Ty>,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    let schema = decl.required_struct()?;
    if schema.generics.len() != arguments.len() {
        return Err(format!(
            "{}: struct application requires {} complete type arguments",
            decl.name,
            schema.generics.len()
        ));
    }
    if arguments
        .iter()
        .any(|ty| !ordinary(ty) && !ty.publication())
    {
        return Err("struct type arguments require canonical ordinary value types".into());
    }
    if exported(decl) {
        for (_, field) in &schema.fields {
            exported_name(library, &decl.module, field, &schema.generics)?;
        }
    }
    let bindings = schema
        .generics
        .into_iter()
        .zip(arguments.clone())
        .collect::<BTreeMap<_, _>>();
    if let Some((parameter, domain)) = schema.type_domain
        && !bindings.get(&parameter).is_some_and(|ty| {
            domain
                .iter()
                .any(|name| Ty::parse(name).as_ref() == Some(ty))
        })
    {
        return Err(format!("requires {parameter} in {{{}}}", domain.join(", ")));
    }
    let origin = format!("{}::{}", decl.module, decl.name);
    if !active.insert(origin.clone()) {
        return Err("recursive ordinary structs are not supported".into());
    }
    let mut fields = Vec::new();
    for (name, ty) in schema.fields {
        let ty = substitute(library, &decl.module, &ty, &bindings, active)?;
        if !ordinary(&ty) && !ty.publication() {
            return Err("ordinary struct fields require ordinary value types".into());
        }
        fields.push((name, ty));
    }
    active.remove(&origin);
    Ok(Ty::Struct(Nominal { origin, arguments }, fields))
}
/// Unify one declaration-owned type pattern against a checked source type.
pub fn unify(
    library: &Library,
    module: &str,
    pattern: &str,
    actual: &Ty,
    generics: &[String],
    bindings: &mut BTreeMap<String, Ty>,
) -> Result<(), String> {
    let normalized = hgl_type_sizes::normalize(pattern, &mut |expr| size(library, module, expr))?;
    let pattern = normalized.as_str();
    if generics.iter().any(|name| name == pattern) {
        if !ordinary(actual) && !actual.publication() {
            return Err("struct type arguments require canonical ordinary value types".into());
        }
        if bindings
            .insert(pattern.into(), actual.clone())
            .is_some_and(|old| old != *actual)
        {
            return Err(format!("conflicting struct inference for {pattern}"));
        }
        return Ok(());
    }
    if let Some(origin) = delta_argument(pattern) {
        let actual_origin = if let Ty::Delta(origin) = actual {
            origin.as_ref()
        } else {
            actual
        };
        if actual_origin.clone().delta()? != *actual {
            return Err("delta originating shape mismatch".into());
        }
        return unify(library, module, origin, actual_origin, generics, bindings);
    }
    if let Some((element, size)) = Ty::list_parts(pattern) {
        let Ty::List(child, actual_size) = actual else {
            return Err("struct field type mismatch".into());
        };
        if size != *actual_size {
            return Err("struct field list fixedness mismatch".into());
        }
        return unify(library, module, element, child, generics, bindings);
    }
    if let Some((base, arguments)) = application(pattern) {
        let children = match (base, actual) {
            ("map", Ty::Map(key, child)) => Some(vec![key.as_ref(), child.as_ref()]),
            ("tuple", Ty::Tuple(children)) => Some(children.iter().collect()),
            ("set", Ty::Set(member)) => Some(vec![member.as_ref()]),
            _ => None,
        };
        if let Some(children) = children {
            if arguments.len() != children.len() {
                return Err("structural type arity mismatch".into());
            }
            for (pattern, actual) in arguments.into_iter().zip(children) {
                unify(library, module, pattern, actual, generics, bindings)?;
            }
            return Ok(());
        }
        let decl = declaration(library, module, base)?
            .ok_or_else(|| format!("unresolved ordinary type {base}"))?;
        let Ty::Struct(identity, _) = actual else {
            return Err("struct field type mismatch".into());
        };
        if identity.origin != format!("{}::{}", decl.module, decl.name)
            || identity.arguments.len() != arguments.len()
        {
            return Err("struct field nominal specialization mismatch".into());
        }
        for (pattern, actual) in arguments.into_iter().zip(&identity.arguments) {
            unify(library, module, pattern, actual, generics, bindings)?;
        }
        return Ok(());
    }
    if resolve(library, module, pattern, &mut BTreeSet::new())? != *actual {
        return Err("struct field type mismatch".into());
    }
    Ok(())
}
fn exported(decl: &Decl) -> bool {
    decl.tokens
        .first()
        .is_some_and(|token| token.text == "export")
}
fn exported_name(
    library: &Library,
    module: &str,
    name: &str,
    generics: &[String],
) -> Result<(), String> {
    if generics.iter().any(|generic| generic == name) {
        return Ok(());
    }
    if let Some(origin) = delta_argument(name) {
        return exported_name(library, module, origin, generics);
    }
    let (base, args) = application(name).unwrap_or((name, Vec::new()));
    for argument in args
        .iter()
        .take(if base == "list" { 1 } else { args.len() })
    {
        exported_name(library, module, argument, generics)?;
    }
    if Ty::parse(name).is_some()
        || matches!(
            base,
            "list" | "map" | "tuple" | "set" | "ref" | "rolling" | "atomic"
        )
    {
        return Ok(());
    }
    if let Some(decl) = declaration(library, module, base)?
        && !exported(decl)
    {
        return Err(format!(
            "exported struct reaches unexported {}::{}",
            decl.module, decl.name
        ));
    }
    Ok(())
}

fn size(library: &Library, module: &str, expr: &str) -> Result<hgl_source::Literal, String> {
    library
        .type_sizes
        .get(&(module.into(), expr.into()))
        .map_or_else(
            || hgl_type_sizes::literal(expr),
            |size| Ok(hgl_source::Literal::Int(*size)),
        )
}
