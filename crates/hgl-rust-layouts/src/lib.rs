//! Checked Rust type spellings and ordinary nominal storage layouts.
use hgl_source::Ty;
mod delta;
mod globals;
pub use delta::{delta_storage, delta_type};
pub use globals::{global_markers, global_schema, global_type, global_types};
pub use hgl_rust_families::{family_coerce, family_member, family_storage};

pub use hgl_rust_scalars::{rust_type, scalar_type};
/// Rust owned representation of a concrete ordinary type.
pub fn owned_type(ty: &Ty) -> String {
    if let Ty::Family(family) = ty {
        return owned_type(&family_storage(family));
    }
    if matches!(ty, Ty::Recursive(_)) {
        return format!("Owned{}", global_type(ty));
    }
    if let Ty::Tuple(children) = ty {
        return owned_type(&tuple_storage(children));
    }
    if let Ty::Delta(origin) = ty {
        return owned_type(&delta_storage(origin));
    }
    if let Some(element) = hgl_rust_collections::element(ty) {
        return format!("Vec<{}>", owned_type(&element));
    }
    if let Ty::Struct(_, fields, optional) = ty {
        return globals::tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
            let ty = owned_type(ty);
            if optional.contains(&i) {
                format!("Option<{ty}>")
            } else {
                ty
            }
        }));
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
        Vec::new(),
    )
}

/// Complete ordinary payload stored behind a prepared whole-value endpoint.
pub fn whole_payload(ty: &Ty) -> Option<&Ty> {
    match ty {
        Ty::Atomic(payload) => Some(payload),
        Ty::Enum(_) => Some(ty),
        Ty::Map(..)
        | Ty::Tuple(_)
        | Ty::Delta(_)
        | Ty::List(..)
        | Ty::Struct(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Recursive(_)
        | Ty::Family(_)
        | Ty::Void => None,
    }
}
