use crate::layouts::{delta_storage, delta_type, rust_type, scalar_type};
use hgl_semantics::ir::Plan;
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Rust marker identifying an exact prepared entry type.
pub fn global_type(ty: &Ty) -> String {
    if let Ty::Native(scalar) = ty {
        return format!(
            "hgl_store::native_values::Native<{},NativeDomain{}>",
            crate::native_values::marker(scalar),
            crate::identifiers::identifier(&scalar.identity)
        );
    }
    if *ty == Ty::Any {
        return "hgl_store::Any".into();
    }
    if let Some(storage) = crate::layouts::ordinary_storage(ty) {
        return global_type(&storage);
    }
    if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
        return nominal_marker(&format!("\0{}", ty.source_name()).into());
    }
    if let Ty::Enum(identity) = ty {
        return crate::enums::marker_type(identity);
    }

    if let Ty::List(element, size) = ty {
        return format!(
            "hgl_store::List<{}, {}>",
            global_type(element),
            size.map_or(-1_i128, |size| size as i128)
        );
    }
    if let Ty::Recursive(batch) = ty {
        return nominal_marker(batch.identity());
    }
    if let Ty::Struct(name, _, _) = ty {
        nominal_marker(name)
    } else {
        rust_type(ty)
    }
}
fn nominal_marker(name: &hgl_source::Nominal) -> String {
    format!(
        "GlobalStruct{}",
        crate::layouts::identifier(&name.source_name())
    )
}
/// Construction-only exact ordinary type descriptor.
pub fn global_schema(ty: &Ty) -> String {
    if let Ty::Recursive(batch) = ty
        && batch.definitions().is_empty()
    {
        return format!(
            "hgl_types::OrdinaryType::RecursiveReference({:?})",
            batch.identity().source_name()
        );
    }
    if ty.scalar_kind().is_some() {
        format!("hgl_types::ScalarType::{}.into()", scalar_type(ty))
    } else {
        format!("<{} as hgl_store::GlobalValue>::schema()", global_type(ty))
    }
}
/// Collect complete nominal definitions reached by this plan's prepared entries.
pub fn global_types(plan: &Plan) -> BTreeMap<String, Ty> {
    let mut types = BTreeMap::new();
    for ty in &plan.ordinary_types {
        collect(ty, &mut types);
    }
    for value in &plan.wiring {
        value_types(value, &mut types);
    }
    for node in &plan.nodes {
        for ty in std::iter::once(&node.result).chain(node.inputs.iter().map(|(_, _, ty)| ty)) {
            if ty.publication() {
                collect(&delta_type(ty), &mut types);
            }
        }
        for value in &node.configuration {
            value_types(value, &mut types);
        }
        hgl_semantics::ir_visit::hooks(node, &mut |v| value_type(v, &mut types));
        for (_, ty) in &node.globals {
            collect(ty, &mut types);
        }
    }
    types
}
/// Emit nominal value layouts reached by this plan's prepared entries.
pub fn global_markers(plan: &Plan) -> String {
    let mut emitted = std::collections::BTreeSet::new();
    global_types(plan)
        .values()
        .filter(|ty| {
            matches!(
                ty,
                Ty::Struct(..)
                    | Ty::Recursive(_)
                    | Ty::Family(_)
                    | Ty::Enum(_)
                    | Ty::Set(_)
                    | Ty::Map(..)
            )
        })
        .filter(|ty| emitted.insert(global_type(ty)))
        .enumerate()
        .map(|(domain, ty)| {
            if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
                crate::collections::marker(ty, global_type, global_schema)
            } else if let Ty::Enum(identity) = ty {
                crate::enums::marker(identity)
            } else {
                let schema = crate::layouts::ordinary_storage(ty).unwrap_or_else(|| ty.clone());
                crate::structs::marker(&schema, global_type, global_schema)
                    + &if crate::composite_keys::composite(ty) {
                        crate::composite_keys::implementation(ty, domain, global_type)
                    } else {
                        String::new()
                    }
            }
        })
        .collect()
}
fn collect(ty: &Ty, types: &mut BTreeMap<String, Ty>) {
    if !matches!(
        ty,
        Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Atomic(_)
            | Ty::Rolling(..)
            | Ty::Ref(_)
            | Ty::Void
            | Ty::Nullable(_)
    ) {
        types.insert(ty.source_name(), ty.clone());
    }
    if let Ty::Family(family) = ty {
        if types
            .insert(family.identity().source_name(), ty.clone())
            .is_none()
        {
            collect(&crate::layouts::family_storage(family), types);
            for (_, member) in family.members() {
                collect(member, types);
            }
        }
        return;
    }
    if let Ty::Recursive(batch) = ty {
        for definition in batch.definitions() {
            let name = definition.identity().source_name();
            if types.contains_key(&name) {
                continue;
            }
            let rooted = hgl_source::RecursiveType::new(
                definition.identity().clone(),
                batch.definitions().to_vec(),
            )
            .unwrap_or_else(|_| unreachable!("validated finite batch"));
            types.insert(name, Ty::Recursive(rooted));
            for (_, field) in definition.fields() {
                collect(field, types);
            }
        }
        return;
    }
    if let Ty::Enum(identity) = ty {
        types.insert(identity.origin.clone(), ty.clone());
    }
    if let Ty::Tuple(children) = ty {
        collect(&crate::layouts::tuple_storage(children), types);
    }
    if let Ty::Delta(origin) = ty {
        collect(&delta_storage(origin), types);
    }
    if let Ty::Ref(origin) = ty
        && origin.publication()
    {
        collect(&delta_type(origin), types);
    }
    if let Some(element) = crate::collections::element(ty) {
        collect(&element, types);
    }
    if let Ty::Struct(name, fields, _) = ty {
        types.insert(name.source_name(), ty.clone());
        for (_, ty) in fields {
            collect(ty, types);
        }
    }
}
pub(crate) fn tuple(fields: impl Iterator<Item = String>) -> String {
    match fields.collect::<Vec<_>>().as_slice() {
        [] => "()".into(),
        fields => format!("({},)", fields.join(",")),
    }
}

fn value_type(value: &hgl_semantics::ir::Value, types: &mut BTreeMap<String, Ty>) -> bool {
    use hgl_semantics::ir::Kind;
    if !matches!(
        value.kind,
        Kind::Wire(_) | Kind::IterationInput(_) | Kind::Input(..) | Kind::Output
    ) {
        collect(&value.ty, types);
    }
    if value.snapshot {
        collect(&crate::snapshots::storage(&value.ty), types);
    }
    true
}
fn value_types(value: &hgl_semantics::ir::Value, types: &mut BTreeMap<String, Ty>) {
    hgl_semantics::ir_visit::value(value, &mut |value| value_type(value, types));
}

/// Collect payload layouts hidden behind prepared harness box types before graph startup.
pub fn harness_types(tests: &[hgl_semantics::harness_ir::Test]) -> BTreeMap<String, Ty> {
    let mut types = BTreeMap::new();
    hgl_semantics::ir_visit::harness(tests, &mut |v| value_type(v, &mut types));
    types
}
