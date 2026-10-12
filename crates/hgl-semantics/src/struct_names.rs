//! Shared nominal declaration lookup and export visibility closure.
use crate::library::{Decl, Library, Role};
use hgl_source::TypeSpelling;
use hgl_source::{Ty, application, delta_argument};
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
    find(library, module, name, &Role::Struct)
}
/// Find a visible nominal enum declaration.
pub fn enum_declaration<'a>(
    library: &'a Library,
    module: &str,
    name: &str,
) -> Result<Option<&'a Decl>, String> {
    find(library, module, name, &Role::Enum)
}
/// Find a visible opaque native scalar declaration through ordinary imports.
pub fn native_declaration<'a>(
    library: &'a Library,
    module: &str,
    name: &str,
) -> Result<Option<&'a Decl>, String> {
    let base = application(name).map_or(name, |(base, _)| base);
    let declaration = find(library, module, base, &Role::NativeType)?;
    if declaration.is_some() && base != name {
        return Err("native scalar type arguments are not admitted".into());
    }
    Ok(declaration)
}
/// Find a visible nominal declaration in the shared type namespace.
pub fn nominal_declaration<'a>(
    library: &'a Library,
    module: &str,
    name: &str,
) -> Result<Option<&'a Decl>, String> {
    let base = application(name).map_or(name, |(base, _)| base);
    Ok(declaration(library, module, base)?
        .or(enum_declaration(library, module, base)?)
        .or(native_declaration(library, module, name)?))
}
fn nominal(role: &Role) -> bool {
    matches!(role, Role::Struct | Role::Enum | Role::NativeType)
}
fn find<'a>(
    library: &'a Library,
    module: &str,
    name: &str,
    role: &Role,
) -> Result<Option<&'a Decl>, String> {
    let local = !name.contains("::")
        && library
            .declarations
            .iter()
            .any(|d| d.module == module && d.name == name && nominal(&d.role));
    let (owner, item) = if local {
        (module.into(), name.into())
    } else {
        identity(library, module, name)
    };
    let mut found = library
        .declarations
        .iter()
        .filter(|d| d.module == owner && d.name == item && &d.role == role);
    let declaration = found.next();
    if found.next().is_some() {
        let kind = if *role == Role::Struct {
            "struct"
        } else {
            "nominal type"
        };
        return Err(format!("duplicate {kind} declaration {owner}::{item}"));
    }
    if let Some(decl) = declaration {
        if decl.module != module && !exported(decl) {
            return Err(format!(
                "{}::{}: type is not exported",
                decl.module, decl.name
            ));
        }
        if let Some((alias, _)) = name.split_once("::")
            && !library.imports.contains_key(&(module.into(), alias.into()))
        {
            return Err(format!(
                "type qualification requires an imported module alias {alias}"
            ));
        }
    }
    Ok(declaration)
}
/// Validate the declaration-owned exported field and generic-argument closure.
pub fn exported_fields(library: &Library, decl: &Decl) -> Result<(), String> {
    if exported(decl) {
        let schema = decl.required_struct()?;
        for (_, field) in &schema.fields {
            exported_name(library, &decl.module, field, &schema.generics)?;
        }
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
    if let Some(decl) = nominal_declaration(library, module, name)?
        && !exported(decl)
    {
        return Err(format!(
            "exported struct reaches unexported {}::{}",
            decl.module, decl.name
        ));
    }
    Ok(())
}
