//! Checked Rust type spellings and ordinary nominal storage layouts.
use hgl_source::Ty;
mod delta;
mod globals;
pub use crate::families::{family_coerce, family_member, family_storage};
pub use delta::{delta_storage, delta_type};
pub(crate) use globals::tuple;
pub use globals::{global_markers, global_schema, global_type, global_types, harness_types};

pub use crate::scalars::{rust_type, scalar_type};
/// Rust owned representation of a concrete ordinary type.
pub fn owned_type(ty: &Ty) -> String {
    if let Some(storage) = ordinary_storage(ty) {
        return owned_type(&storage);
    }
    if matches!(ty, Ty::Recursive(_)) {
        return format!("Owned{}", global_type(ty));
    }

    if let Some(element) = crate::collections::element(ty) {
        return format!("Vec<{}>", owned_type(&element));
    }
    if let Ty::Struct(_, fields, optional) = ty {
        return tuple(fields.iter().enumerate().map(|(i, (_, ty))| {
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
    if let Ty::Atomic(payload) = ty {
        Some(payload)
    } else if matches!(ty, Ty::Any | Ty::Enum(_)) {
        Some(ty)
    } else {
        None
    }
}

/// Lower source-only storage wrappers through their shared ordinary representation.
pub fn ordinary_storage(ty: &Ty) -> Option<Ty> {
    if let Ty::Family(family) = ty {
        return Some(family_storage(family));
    }
    if let Ty::Tuple(children) = ty {
        return Some(tuple_storage(children));
    }
    if let Ty::Delta(origin) = ty {
        return Some(delta_storage(origin));
    }
    None
}

pub(crate) use crate::identifiers::identifier;
