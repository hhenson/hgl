//! Resolve finite ordinary value schemas against source library declarations.
use hgl_library::{Decl, Library, Role};
use hgl_source::{Nominal, Ty, application};
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
    if let Some(ty) = bindings.get(name) {
        return Ok(ty.clone());
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
    if matches!(base, "atomic" | "rolling" | "ref" | "set") {
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
    if arguments.iter().any(|ty| !ordinary(ty)) {
        return Err("struct type arguments require canonical ordinary value types".into());
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
    if exported(decl) {
        for argument in &arguments {
            exported_type(library, argument)?;
        }
    }
    let mut fields = Vec::new();
    for (name, ty) in schema.fields {
        let ty = substitute(library, &decl.module, &ty, &bindings, active)?;
        if !ordinary(&ty) {
            return Err("ordinary struct fields require ordinary value types".into());
        }
        if exported(decl) {
            exported_type(library, &ty)?;
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
    if generics.iter().any(|name| name == pattern) {
        if !ordinary(actual) {
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
fn exported_type(library: &Library, ty: &Ty) -> Result<(), String> {
    if let Ty::List(element, _) = ty {
        return exported_type(library, element);
    }
    if let Ty::Struct(identity, fields) = ty {
        if !library.declarations.iter().any(|d| {
            d.role == Role::Struct
                && format!("{}::{}", d.module, d.name) == identity.origin
                && exported(d)
        }) {
            return Err(format!("exported struct reaches unexported {identity}"));
        }
        for argument in &identity.arguments {
            exported_type(library, argument)?;
        }
        for (_, field) in fields {
            exported_type(library, field)?;
        }
    }
    Ok(())
}
