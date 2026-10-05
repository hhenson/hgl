//! Resolve finite ordinary value schemas against source library declarations.
use hgl_library::{Decl, Library};
use hgl_source::{Nominal, Ty, application, delta_argument};
pub use hgl_struct_names::{declaration, identity};
use hgl_value_check::ordinary;
use std::collections::{BTreeMap, BTreeSet};
mod family;
pub use family::field_type;
/// Resolve a finite ordinary type, retaining nominal identity.
pub fn resolve(
    library: &Library,
    module: &str,
    name: &str,
    active: &mut BTreeSet<String>,
) -> Result<Ty, String> {
    substitute(library, module, name, &BTreeMap::new(), active)
}
/// Resolve a source type into its ordinary payload representation.
pub fn resolve_ordinary(library: &Library, module: &str, name: &str) -> Result<Ty, String> {
    resolve(library, module, name, &mut BTreeSet::new()).map(|ty| hgl_value_access::project(&ty))
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
    let (base, arguments) = application(name).unwrap_or((name, Vec::new()));
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
    if base == "atomic" && arguments.len() == 1 {
        return Ok(hgl_value_access::project(&substitute(
            library,
            module,
            arguments[0],
            bindings,
            active,
        )?)
        .atomic());
    }
    if matches!(base, "rolling" | "ref") {
        return Err(format!(
            "unsupported ordinary struct argument or field {name}"
        ));
    }
    if let Some(ty) = hgl_enums::resolve(library, module, base)? {
        if !arguments.is_empty() {
            return Err("enum type arguments are unsupported".into());
        }
        return Ok(Ty::Enum(ty));
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
    let (schema, patterns) = hgl_inheritance::schema(library, decl)?;
    if schema.generics.len() != arguments.len() {
        return Err(format!(
            "{}: struct application requires {} complete type arguments",
            decl.name,
            schema.generics.len()
        ));
    }
    if schema.abstract_type {
        return hgl_family_types::resolve(
            library,
            decl,
            &arguments,
            active,
            |decl, args, active| specialize(library, decl, args, active),
            |module, name, bindings| {
                substitute(library, module, name, bindings, &mut BTreeSet::new())
            },
        );
    }
    if let Some(ty) =
        hgl_recursive_types::resolve(library, decl, &arguments, |owner, pattern, bindings| {
            substitute(
                library,
                &owner.module,
                pattern,
                bindings,
                &mut BTreeSet::new(),
            )
        })?
    {
        return Ok(ty);
    }
    hgl_shape_obligations::validate(library, decl, &arguments)?;
    hgl_struct_names::exported_fields(library, decl)?;
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
    for (index, (name, _)) in schema.fields.into_iter().enumerate() {
        let ty = field_type(library, &patterns[index], &bindings, active)?;
        if !ordinary(&ty) && !ty.publication() {
            return Err("ordinary struct fields require ordinary value types".into());
        }
        for (_, value) in schema.defaults.iter().filter(|(field, _)| *field == index) {
            if !matches!(value, hgl_source::Expr::Null)
                && hgl_enums::default(library, &decl.module, value)?.ty()
                    != hgl_value_access::project(&ty)
            {
                return Err(format!("struct field {name}: default type mismatch"));
            }
        }
        fields.push((name, ty));
    }
    active.remove(&origin);
    Ok(Ty::Struct(
        Nominal { origin, arguments },
        fields,
        schema.optional,
    ))
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
        let actual = source_argument(library, actual)?;
        if bindings
            .insert(pattern.into(), actual.clone())
            .is_some_and(|old| old != actual)
        {
            return Err(format!("conflicting struct inference for {pattern}"));
        }
        return Ok(());
    }
    if let Some(origin) = delta_argument(pattern) {
        return unify_delta(library, module, origin, actual, generics, bindings);
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
        if base == "atomic" && arguments.len() == 1 {
            let payload = if let Ty::Atomic(payload) = actual {
                payload.as_ref()
            } else if actual.clone().atomic() == *actual {
                actual
            } else {
                return Err("atomic boundary mismatch".into());
            };
            return unify(library, module, arguments[0], payload, generics, bindings);
        }
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
        if hgl_enums::resolve(library, module, base)?.is_some() {
            return Err("enum type arguments are unsupported".into());
        }
        let decl = declaration(library, module, base)?
            .ok_or_else(|| format!("unresolved ordinary type {base}"))?;
        let identity = if let Ty::Recursive(batch) = actual {
            batch.identity()
        } else if let Ty::Family(family) = actual {
            family.identity()
        } else {
            actual.structure()?.0
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
    let expected = resolve(library, module, pattern, &mut BTreeSet::new())?;
    if expected != *actual && hgl_value_access::project(&expected) != *actual {
        return Err("struct field type mismatch".into());
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

fn unify_delta(
    library: &Library,
    module: &str,
    origin: &str,
    actual: &Ty,
    generics: &[String],
    bindings: &mut BTreeMap<String, Ty>,
) -> Result<(), String> {
    if let Ok(expected) =
        substitute(library, module, origin, bindings, &mut BTreeSet::new()).and_then(Ty::delta)
    {
        return if expected == *actual {
            Ok(())
        } else {
            Err("delta originating shape mismatch".into())
        };
    }
    if let Some(("atomic", arguments)) = application(origin)
        && let [payload] = arguments.as_slice()
    {
        return unify(library, module, payload, actual, generics, bindings);
    }
    let actual_origin = if let Ty::Delta(origin) = actual {
        origin.as_ref()
    } else if actual.atomic_payload()
        && !matches!(actual, Ty::List(..) | Ty::Tuple(_) | Ty::Struct(..))
    {
        actual
    } else {
        return Err("unresolved temporal shape from complete composite delta payload".into());
    };
    if actual_origin.clone().delta()? != *actual {
        return Err("delta originating shape mismatch".into());
    }
    unify(library, module, origin, actual_origin, generics, bindings)
}

fn source_argument(library: &Library, ty: &Ty) -> Result<Ty, String> {
    if let Ok((identity, _, _)) = ty.structure() {
        let missing = "unresolved nominal source argument";
        let (module, name) = identity.origin.rsplit_once("::").ok_or(missing)?;
        let decl = declaration(library, module, name)?.ok_or(missing)?;
        return specialize(
            library,
            decl,
            identity.arguments.clone(),
            &mut BTreeSet::new(),
        );
    }
    if let Ty::List(child, size) = ty {
        return Ok(Ty::List(Box::new(source_argument(library, child)?), *size));
    }
    if let Ty::Tuple(children) = ty {
        return Ok(Ty::Tuple(
            children
                .iter()
                .map(|child| source_argument(library, child))
                .collect::<Result<_, _>>()?,
        ));
    }
    Ok(ty.clone())
}
