//! Checked Rust type spellings and ordinary nominal storage layouts.
use hgl_source::Ty;
mod delta;
mod globals;
pub use delta::{delta_storage, delta_type};
pub use globals::{global_markers, global_schema, global_type};

/// Emit the checked rust type form.
pub fn rust_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "i64",
        Ty::Duration => "hgl_types::EngineDelta",
        Ty::Date => "hgl_types::Date",
        Ty::Time => "hgl_types::Time",
        Ty::DateTime => "hgl_types::EngineTime",
        Ty::Bool => "bool",
        Ty::F64 => "f64",
        Ty::Str => "String",
        Ty::Void => "()",
        Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Nullable(_) => {
            unreachable!("ordinary aggregate and nullable locals use inferred Rust types")
        }
        Ty::Ref(_) => "hgl_store::Reference",
        Ty::Set(_) => "hgl_store::InputId",
    }
}
/// Emit the checked scalar type form.
pub fn scalar_type(ty: &Ty) -> &'static str {
    match ty {
        Ty::Bool => "Bool",
        Ty::F64 => "F64",
        Ty::I64 => "I64",
        Ty::Str => "Text",
        Ty::Duration => "Duration",
        Ty::Date => "Date",
        Ty::Time => "Time",
        Ty::DateTime => "DateTime",
        Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Struct(..)
        | Ty::List(..)
        | Ty::Void => {
            unreachable!("checked endpoint type")
        }
    }
}
/// Rust owned representation of a concrete ordinary type.
pub fn owned_type(ty: &Ty) -> String {
    if let Ty::Tuple(children) = ty {
        return owned_type(&tuple_storage(children));
    }
    if let Ty::Delta(origin) = ty {
        return owned_type(&delta_storage(origin));
    }
    if let Ty::List(element, _) = ty {
        return format!("Vec<{}>", owned_type(element));
    }
    if let Ty::Struct(_, fields) = ty {
        return globals::tuple(fields.iter().map(|(_, ty)| owned_type(ty)));
    }
    rust_type(ty).into()
}

fn tuple_storage(children: &[Ty]) -> Ty {
    let identity = Ty::Tuple(children.to_vec()).source_name();
    Ty::Struct(
        format!("\0{identity}").into(),
        children
            .iter()
            .enumerate()
            .map(|(i, ty)| (i.to_string(), ty.clone()))
            .collect(),
    )
}
