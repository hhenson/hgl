//! Finite recursive specialization batches without recursive type expansion.
use crate::library::{Decl, Library};
use hgl_source::{Nominal, NominalDefinition, RecursiveType, Ty};
use std::collections::{BTreeMap, BTreeSet};
mod component;
/// Resolve a recursive root, delegating ordinary fields to the normal resolver.
pub fn resolve(
    library: &Library,
    root: &Decl,
    arguments: &[Ty],
    mut field: impl FnMut(&Decl, &str, &BTreeMap<String, Ty>) -> Result<Ty, String>,
) -> Result<Option<Ty>, String> {
    let members = component::members(library, root)?;
    if members.is_empty() {
        return Ok(None);
    }
    let identity = Nominal {
        origin: component::identity(root),
        arguments: arguments.to_vec(),
    };
    let mut pending = vec![identity.clone()];
    let mut seen = BTreeSet::new();
    let mut definitions = Vec::new();
    while let Some(current) = pending.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        let decl = members
            .iter()
            .find(|d| component::identity(d) == current.origin)
            .ok_or("unresolved recursive declaration")?;
        let schema = decl.required_struct()?;
        if schema.generics.len() != current.arguments.len() {
            return Err("recursive struct requires complete concrete type arguments".into());
        }
        crate::struct_names::exported_fields(library, decl)?;
        crate::shape_obligations::validate(library, decl, &current.arguments)?;
        let bindings = schema
            .generics
            .iter()
            .cloned()
            .zip(current.arguments.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        if let Some((parameter, domain)) = &schema.type_domain
            && !bindings.get(parameter).is_some_and(|ty| {
                domain
                    .iter()
                    .any(|name| Ty::parse(name).as_ref() == Some(ty))
            })
        {
            return Err(format!("requires {parameter} in {{{}}}", domain.join(", ")));
        }
        let mut fields = Vec::new();
        for (index, (name, pattern)) in schema.fields.iter().enumerate() {
            let ty =
                if let Some((target, args)) = component::edge(library, decl, pattern, &members)? {
                    if !schema.optional.contains(&index) {
                        return Err(
                            "recursive edges require an optional atomic field with a null default"
                                .into(),
                        );
                    }
                    let arguments = args
                        .iter()
                        .map(|arg| field(decl, arg, &bindings))
                        .collect::<Result<Vec<_>, _>>()?;
                    let target = Nominal {
                        origin: component::identity(target),
                        arguments,
                    };
                    pending.push(target.clone());
                    Ty::Recursive(RecursiveType::reference(target))
                } else {
                    crate::value_access::project(&field(decl, pattern, &bindings)?)
                };
            validate_field(library, decl, &schema, index, &ty)?;
            fields.push((name.clone(), ty));
        }
        definitions.push(NominalDefinition::new(current, fields, schema.optional));
    }
    Ok(Some(Ty::Recursive(RecursiveType::new(
        identity,
        definitions,
    )?)))
}

fn validate_field(
    library: &Library,
    decl: &Decl,
    schema: &crate::library::RequiredStruct,
    index: usize,
    ty: &Ty,
) -> Result<(), String> {
    let name = &schema.fields[index].0;
    if !crate::value_access::ordinary(ty) || !ty.atomic_payload() {
        return Err(
            "recursive struct fields require complete ordinary atomic payload types".into(),
        );
    }
    for (_, value) in schema.defaults.iter().filter(|(field, _)| *field == index) {
        if !matches!(value.syntax(), hgl_source::Expr::Null)
            && crate::enums::default(library, &decl.module, value)?.ty() != *ty
        {
            return Err(format!("struct field {name}: default type mismatch"));
        }
    }
    Ok(())
}
