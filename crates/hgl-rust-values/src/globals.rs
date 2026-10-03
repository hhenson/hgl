use crate::{rust_type, scalar_type};
use hgl_rust_ir::Plan;
use hgl_source::Ty;
use std::collections::BTreeMap;

/// Rust marker identifying an exact prepared entry type.
pub fn global_type(ty: &Ty) -> String {
    if let Ty::Struct(name, _) = ty {
        let identity = name
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
    if matches!(ty, Ty::Struct(..)) {
        format!("<{} as hgl_store::GlobalValue>::schema()", global_type(ty))
    } else {
        format!("hgl_types::ScalarType::{}.into()", scalar_type(ty))
    }
}
/// Emit nominal value layouts reached by this plan's prepared entries.
pub fn global_markers(plan: &Plan) -> String {
    let mut types = BTreeMap::new();
    for node in &plan.nodes {
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
    if let Ty::Struct(name, fields) = ty {
        types.insert(name.clone(), ty);
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
        .map(|(i, _)| format!("slots.{i}.commit(columns, value.{i});"))
        .collect::<Vec<_>>()
        .concat();
    format!(
        "#[derive(Debug)]\nstruct {name};\nimpl hgl_store::GlobalValue for {name} {{\ntype Value = {value};\ntype Slots = {slots};\nfn schema() -> hgl_types::OrdinaryType {{ hgl_types::OrdinaryType::Struct({identity:?}, vec![{schema}]) }}\nfn slots(layout: &mut &[usize]) -> Self::Slots {{ {bind} }}\nfn retain(value: &Self::Value) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({retain}) }}\nfn read(columns: &hgl_store::Columns, slots: Self::Slots) -> Result<Self::Value, Box<hgl_types::NodeError>> {{ Ok({read}) }}\nfn commit(columns: &mut hgl_store::Columns, slots: Self::Slots, value: Self::Value) {{ {commit} }}\n}}\n"
    )
}
