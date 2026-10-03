//! Resolve finite ordinary value schemas against source library declarations.
use hgl_library::{Decl, Library, Role};
use hgl_source::Ty;
use hgl_value_check::ordinary;
use std::collections::BTreeSet;
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
    if let Some(ty) = Ty::parse(name) {
        return Ok(ty);
    }
    if let Some((element, size)) = Ty::list_parts(name) {
        return Ok(Ty::List(
            Box::new(resolve(library, module, element, active)?),
            size,
        ));
    }
    let decl = declaration(library, module, name)?
        .ok_or_else(|| format!("unresolved ordinary type {name}"))?;
    let identity = format!("{}::{}", decl.module, decl.name);
    if !active.insert(identity.clone()) {
        return Err("recursive ordinary structs are not supported".into());
    }
    let mut fields = Vec::new();
    for (name, ty) in decl.required_fields()? {
        let ty = resolve(library, &decl.module, &ty, active)?;
        if !ordinary(&ty) {
            return Err(
                "ordinary struct fields require primitives or required-field structs".into(),
            );
        }
        if exported(decl) {
            exported_type(library, &ty)?;
        }
        fields.push((name, ty));
    }
    active.remove(&identity);
    Ok(Ty::Struct(identity, fields))
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
    if let Ty::Struct(identity, _) = ty
        && !library.declarations.iter().any(|d| {
            d.role == Role::Struct
                && format!("{}::{}", d.module, d.name) == *identity
                && exported(d)
        })
    {
        return Err(format!("exported struct reaches unexported {identity}"));
    }
    Ok(())
}
