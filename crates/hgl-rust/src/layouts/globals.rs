use crate::layouts::{delta_storage, delta_type, rust_type, scalar_type};
use hgl_semantics::ir::Plan;
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Rust marker identifying an exact prepared entry type.
pub fn global_type(ty: &Ty) -> String {
    if let Ty::Family(family) = ty {
        return global_type(&crate::layouts::family_storage(family));
    }
    if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
        return nominal_marker(&format!("\0{}", ty.source_name()).into());
    }
    if let Ty::Enum(identity) = ty {
        return crate::enums::marker_type(identity);
    }
    if let Ty::Tuple(children) = ty {
        return global_type(&crate::layouts::tuple_storage(children));
    }
    if let Ty::Delta(origin) = ty {
        return global_type(&delta_storage(origin));
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
        rust_type(ty).into()
    }
}
fn nominal_marker(name: &hgl_source::Nominal) -> String {
    let identity = name
        .source_name()
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .concat();
    format!("GlobalStruct{identity}")
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
    if matches!(
        ty,
        Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Enum(_)
            | Ty::Tuple(_)
            | Ty::Delta(_)
            | Ty::Struct(..)
            | Ty::List(..)
            | Ty::Set(_)
            | Ty::Map(..)
    ) {
        format!("<{} as hgl_store::GlobalValue>::schema()", global_type(ty))
    } else {
        format!("hgl_types::ScalarType::{}.into()", scalar_type(ty))
    }
}
/// Collect complete nominal definitions reached by this plan's prepared entries.
pub fn global_types(plan: &Plan) -> BTreeMap<String, Ty> {
    let mut types = BTreeMap::new();
    for node in &plan.nodes {
        for ty in std::iter::once(&node.result).chain(node.inputs.iter().map(|(_, _, ty)| ty)) {
            if ty.publication() {
                collect(&delta_type(ty), &mut types);
            }
        }
        for value in &node.configuration {
            value_types(value, &mut types);
        }
        if let Some(body) = &node.generator {
            statement_types(body, &mut types);
        }
        statement_types(&node.start, &mut types);
        statement_types(&node.stop, &mut types);
        for (guard, body) in &node.handlers {
            if let Some(guard) = guard {
                value_types(guard, &mut types);
            }
            statement_types(body, &mut types);
        }
        for (_, ty) in &node.globals {
            collect(ty, &mut types);
        }
    }
    types
}
/// Emit nominal value layouts reached by this plan's prepared entries.
pub fn global_markers(plan: &Plan) -> String {
    global_types(plan)
        .values()
        .enumerate()
        .map(|(domain, ty)| {
            if let Ty::Family(family) = ty {
                crate::structs::marker(
                    &crate::layouts::family_storage(family),
                    global_type,
                    global_schema,
                )
            } else if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
                crate::collections::marker(ty, global_type, global_schema)
            } else if let Ty::Enum(identity) = ty {
                crate::enums::marker(identity)
            } else {
                crate::structs::marker(ty, global_type, global_schema)
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
    if let Ty::Family(family) = ty {
        if types
            .insert(family.identity().source_name(), ty.clone())
            .is_none()
        {
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
    if matches!(ty, Ty::Set(_) | Ty::Map(..)) {
        types.insert(ty.source_name(), ty.clone());
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
pub(super) fn tuple(fields: impl Iterator<Item = String>) -> String {
    match fields.collect::<Vec<_>>().as_slice() {
        [] => "()".into(),
        fields => format!("({},)", fields.join(",")),
    }
}

fn statement_types(statements: &[hgl_semantics::ir::Statement], types: &mut BTreeMap<String, Ty>) {
    use hgl_semantics::ir::Statement;
    for statement in statements {
        match statement {
            Statement::Let(_, value)
            | Statement::Var(_, value)
            | Statement::Borrow(_, value, _)
            | Statement::Return(value)
            | Statement::Yield(value)
            | Statement::Call(value) => value_types(value, types),
            Statement::TimedYield(target, value) | Statement::Assign(target, value) => {
                value_types(target, types);
                value_types(value, types);
            }
            Statement::While(value, body)
            | Statement::ForItems(_, _, _, value, body)
            | Statement::For(_, value, body) => {
                value_types(value, types);
                statement_types(body, types);
            }
            Statement::If(value, yes, no) => {
                value_types(value, types);
                statement_types(yes, types);
                statement_types(no, types);
            }
            Statement::Exit => {}
        }
    }
}
fn value_types(value: &hgl_semantics::ir::Value, types: &mut BTreeMap<String, Ty>) {
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
    match &value.kind {
        Kind::List(values) | Kind::Native(_, values) | Kind::Query(_, values) => {
            for value in values {
                value_types(value, types);
            }
        }
        Kind::Delta(entries) => {
            for entry in entries {
                for v in entry.operands() {
                    value_types(v, types);
                }
            }
        }
        Kind::Construct(fields) => {
            for (_, value) in fields {
                value_types(value, types);
            }
        }
        Kind::ValueCall(args, body) => {
            for value in args {
                value_types(value, types);
            }
            statement_types(body, types);
        }
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            value_types(a, types);
            value_types(b, types);
        }
        Kind::Length(v)
        | Kind::Field(v, _)
        | Kind::GlobalSet(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v)
        | Kind::Unary(_, v) => value_types(v, types),
        Kind::WiringFailure(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Configuration(_)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::ObservedLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::TemporalLiteral(_)
        | Kind::Captured(..)
        | Kind::Prepared(_)
        | Kind::Void => {}
    }
}
