//! Static ordinary shape capabilities and field metadata, independent of values or execution.
use crate::{FamilyType, Ty};
/// Static capability declarations; contained boxes retain their claimed capabilities.
pub fn capable(ty: &Ty, operation: usize) -> bool {
    match ty {
        Ty::Delta(_) | Ty::Void | Ty::Ref(_) | Ty::Atomic(_) | Ty::Rolling(..) => false,
        Ty::Set(child) => operation != 2 && capable(child, 0) && capable(child, 1),
        Ty::Map(key, value) => {
            operation != 2 && capable(key, 0) && capable(key, 1) && capable(value, operation)
        }
        Ty::List(child, _) | Ty::Nullable(child) => capable(child, operation),
        Ty::Tuple(fields) => fields.iter().all(|ty| capable(ty, operation)),
        Ty::Struct(_, fields, _) => fields.iter().all(|(_, ty)| capable(ty, operation)),
        Ty::Family(family) => family
            .members()
            .iter()
            .all(|(_, member)| capable(member, operation)),
        Ty::Recursive(batch) => batch.definitions().iter().all(|definition| {
            definition
                .fields()
                .iter()
                .all(|(_, ty)| capable(ty, operation))
        }),
        Ty::Bool | Ty::TimeZone | Ty::ZonedTime | Ty::ZonedDateTime => operation != 2,
        Ty::Any
        | Ty::I64
        | Ty::F64
        | Ty::Str
        | Ty::Bytes
        | Ty::Enum(_)
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::Duration => true,
    }
}
/// Declaration-ordered ordinary field metadata, preserving optional positions.
pub fn fields(ty: &Ty) -> Option<(Vec<Ty>, Vec<usize>)> {
    if let Ty::Tuple(fields) = ty {
        return Some((fields.clone(), Vec::new()));
    }
    ty.structure().ok().map(|(_, fields, optional)| {
        (
            fields.iter().map(|(_, ty)| ty.clone()).collect(),
            optional.to_vec(),
        )
    })
}

/// Whether an admitted ordinary shape recursively contains owning boxes.
pub fn contains_any(ty: &Ty) -> bool {
    match ty {
        Ty::Any => true,
        Ty::Atomic(t)
        | Ty::Delta(t)
        | Ty::Rolling(t, _)
        | Ty::List(t, _)
        | Ty::Set(t)
        | Ty::Ref(t)
        | Ty::Nullable(t) => contains_any(t),
        Ty::Map(k, v) => contains_any(k) || contains_any(v),
        Ty::Tuple(fields) => fields.iter().any(contains_any),
        Ty::Struct(_, fields, _) => fields.iter().any(|(_, ty)| contains_any(ty)),
        Ty::Recursive(batch) => batch
            .definitions()
            .iter()
            .any(|d| d.fields().iter().any(|(_, ty)| contains_any(ty))),
        Ty::Family(family) => family.members().iter().any(|(_, ty)| contains_any(ty)),
        Ty::Enum(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => false,
    }
}

/// Fixed typed discriminator and member slots for a closed nominal family.
pub fn family_storage(family: &FamilyType) -> Ty {
    let mut fields = vec![("tag".into(), Ty::I64)];
    fields.extend(
        family
            .members()
            .iter()
            .map(|(id, ty)| (id.source_name(), ty.clone())),
    );
    Ty::Struct(
        format!("\0family<{}>", family.identity().source_name()).into(),
        fields,
        (1..=family.members().len()).collect(),
    )
}
