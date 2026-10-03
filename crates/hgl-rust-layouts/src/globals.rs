use crate::{rust_type, scalar_type};
use hgl_rust_ir::Plan;
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Rust marker identifying an exact prepared entry type.
pub fn global_type(ty: &Ty) -> String {
    if let Ty::List(element, size) = ty {
        return format!(
            "hgl_store::List<{}, {}>",
            global_type(element),
            size.map_or(-1_i128, |size| size as i128)
        );
    }
    if let Ty::Struct(name, _) = ty {
        let identity = name
            .source_name()
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .concat();
        format!("GlobalStruct{identity}")
    } else {
        rust_type(ty).into()
    }
}
/// Construction-only exact ordinary type descriptor.
pub fn global_schema(ty: &Ty) -> String {
    if matches!(ty, Ty::Struct(..) | Ty::List(..)) {
        format!("<{} as hgl_store::GlobalValue>::schema()", global_type(ty))
    } else {
        format!("hgl_types::ScalarType::{}.into()", scalar_type(ty))
    }
}
/// Emit nominal value layouts reached by this plan's prepared entries.
pub fn global_markers(plan: &Plan) -> String {
    let mut types = BTreeMap::new();
    for node in &plan.nodes {
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
        .values()
        .map(|ty| marker(ty))
        .collect::<Vec<_>>()
        .concat()
}
fn collect<'a>(ty: &'a Ty, types: &mut BTreeMap<String, &'a Ty>) {
    if let Ty::List(element, _) = ty {
        collect(element, types);
    }
    if let Ty::Struct(name, fields) = ty {
        types.insert(name.source_name(), ty);
        for (_, ty) in fields {
            collect(ty, types);
        }
    }
}
fn tuple(fields: impl Iterator<Item = String>) -> String {
    let fields = fields.collect::<Vec<_>>();
    if fields.is_empty() {
        "()".into()
    } else {
        format!("({},)", fields.join(","))
    }
}
fn marker(ty: &Ty) -> String {
    let Ty::Struct(identity, fields) = ty else {
        unreachable!("collected structs")
    };
    let identity = identity.source_name();
    let name = global_type(ty);
    let value = tuple(
        fields
            .iter()
            .map(|(_, ty)| format!("<{} as hgl_store::GlobalValue>::Value", global_type(ty))),
    );
    let slots = tuple(
        fields
            .iter()
            .map(|(_, ty)| format!("hgl_store::ValueSlot<{}>", global_type(ty))),
    );
    let schema = fields
        .iter()
        .map(|(name, ty)| format!("({name:?}, {})", global_schema(ty)))
        .collect::<Vec<_>>()
        .join(",");
    let bind = tuple(
        fields
            .iter()
            .map(|(_, ty)| format!("hgl_store::ValueSlot::<{}>::bind(layout)", global_type(ty))),
    );
    let retain = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "<{} as hgl_store::GlobalValue>::retain(&value.{i})?",
            global_type(ty)
        )
    }));
    let read = tuple(
        fields
            .iter()
            .enumerate()
            .map(|(i, _)| format!("slots.{i}.read(columns)?")),
    );
    let commit = fields
        .iter()
        .enumerate()
        .map(|(i, _)| format!("slots.{i}.commit(columns, value.{i}, layouts);"))
        .collect::<Vec<_>>()
        .concat();
    let widths = fields
        .iter()
        .map(|(_, ty)| format!("<{} as hgl_store::GlobalValue>::WIDTH", global_type(ty)))
        .collect::<Vec<_>>();
    let width = if widths.is_empty() {
        "0".into()
    } else {
        widths.join("+")
    };
    let prepare = fields
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| {
            format!(
                "<{} as hgl_store::GlobalValue>::prepare(&value.{i}, capacity, layouts)?;",
                global_type(ty)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let install = tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
        format!(
            "hgl_store::ValueSlot::<{}>::install(columns,value.{i},layouts)",
            global_type(ty)
        )
    }));
    let release = fields
        .iter()
        .enumerate()
        .map(|(i, _)| format!("slots.{i}.release(columns);"))
        .collect::<Vec<_>>()
        .concat();
    let flatten = widths.iter().enumerate().map(|(i,width)| format!("let (field, rest) = layout.split_at_mut({width}); slots.{i}.flatten(field); let layout = rest;")).collect::<Vec<_>>().concat();
    format!(
        "#[derive(Debug)]\nstruct {name};\nimpl hgl_store::GlobalValue for {name} {{\ntype Value = {value};\ntype Slots = {slots};\nconst WIDTH: usize = {width};\nfn prepare(value: &Self::Value, capacity: &mut hgl_store::Capacity, layouts: &mut hgl_store::Layouts) -> hgl_types::NodeResult {{ {prepare} Ok(()) }}\nfn install(columns: &mut hgl_store::ValueColumns, value: Self::Value, layouts: &mut hgl_store::Layouts) -> Self::Slots {{ {install} }}\nfn release(columns: &mut hgl_store::ValueColumns, slots: Self::Slots) {{ {release} }}\nfn flatten(slots: Self::Slots, layout: &mut [usize]) {{ {flatten} }}\nfn schema() -> hgl_types::OrdinaryType {{ hgl_types::OrdinaryType::Struct({identity:?}, vec![{schema}]) }}\nfn slots(layout: &mut &[usize]) -> Self::Slots {{ {bind} }}\nfn retain(value: &Self::Value) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({retain}) }}\nfn read(columns: &hgl_store::ValueColumns, slots: Self::Slots) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({read}) }}\nfn commit(columns: &mut hgl_store::ValueColumns, slots: Self::Slots, value: Self::Value, layouts: &mut hgl_store::Layouts) {{ {commit} }}\n}}\n"
    )
}

fn statement_types<'a>(
    statements: &'a [hgl_rust_ir::Statement],
    types: &mut BTreeMap<String, &'a Ty>,
) {
    use hgl_rust_ir::Statement;
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
            Statement::While(value, body) | Statement::For(_, value, body) => {
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
fn value_types<'a>(value: &'a hgl_rust_ir::Value, types: &mut BTreeMap<String, &'a Ty>) {
    use hgl_rust_ir::Kind;
    collect(&value.ty, types);
    match &value.kind {
        Kind::List(values) | Kind::Native(_, values) | Kind::Query(_, values) => {
            for value in values {
                value_types(value, types);
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
        | Kind::ReplaySlot(v)
        | Kind::IsPresent(v)
        | Kind::Present(v)
        | Kind::Unary(_, v) => value_types(v, types),
        Kind::WiringFailure(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Configuration(_)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
}
