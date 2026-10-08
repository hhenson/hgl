//! Ordinary owning, lexical global and evaluation-local observation authority.
use crate::ir::{Kind, Value};
use hgl_source::Ty;
/// Whether the checked type admits ordinary owning retention.
pub fn ordinary(ty: &Ty) -> bool {
    if matches!(ty, Ty::Recursive(_) | Ty::Family(_)) {
        return true;
    }
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().all(|(_, ty)| ordinary(&project(ty)));
    }
    if let Ty::Tuple(children) = ty {
        return children.iter().all(ordinary);
    }
    if let Ty::Set(key) = ty {
        return ordinary(key) && project(key).collection_key();
    }
    if let Ty::Map(key, value) = ty {
        return ordinary(key)
            && project(key).collection_key()
            && ordinary(value)
            && project(value).atomic_payload();
    }
    if let Ty::List(element, _) = ty {
        return ordinary(element);
    }
    matches!(
        ty,
        Ty::Bool
            | Ty::I64
            | Ty::F64
            | Ty::Str
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::Enum(_)
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Duration
            | Ty::Delta(_)
    )
}
/// Whether a checked place carries recursive write authority.
pub fn writable(value: &Value) -> bool {
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return writable(parent);
    }
    matches!(
        value.kind,
        Kind::MutableLocal(_) | Kind::BorrowedLocal(_, _, true)
    )
}
/// Resolve a declared ordinary field without changing its parent's authority.
pub fn field(parent: Value, name: &str) -> Result<Value, String> {
    let schema = if let Ty::Family(family) = &parent.ty {
        family
            .members()
            .first()
            .map(|(_, ty)| ty)
            .ok_or("empty family has no field")?
    } else {
        &parent.ty
    };
    let (_, fields, optional) = schema.structure().map_err(|error| {
        format!("field access requires an ordinary struct or direct injected clock: {error}")
    })?;
    if matches!(parent.kind, Kind::Wire(_) | Kind::Output)
        || (matches!(parent.kind, Kind::Input(..) | Kind::IterationInput(_))
            && !matches!(parent.ty, Ty::Struct(..)))
    {
        return Err(
            "temporal child projection is outside the admitted publication-delta profile".into(),
        );
    }
    let (index, (_, ty)) = fields
        .iter()
        .enumerate()
        .find(|(_, (field, _))| field == name)
        .ok_or_else(|| format!("unknown struct field {name}"))?;
    if optional.contains(&index) {
        return Err("optional field access is outside the admitted value profile".into());
    }
    let snapshot = parent.snapshot;
    let mut value = Value::new(ty.clone(), Kind::Field(Box::new(parent), index));
    value.snapshot = snapshot;
    Ok(value)
}
/// Return the entry and access mode carried by an aggregate view.
pub fn provenance(value: &Value) -> Option<(usize, bool)> {
    if !aggregate(&value.ty) {
        return None;
    }
    if let Kind::BorrowedLocal(_, entry, writable) = value.kind {
        return Some((entry, writable));
    }
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return provenance(parent);
    }
    None
}
/// Bind an owning value or preserve an explicitly borrowed initializer.
pub fn binding(id: usize, value: &Value, mutable: bool, annotated: bool) -> Result<Value, String> {
    let kind = if observed(value) {
        if mutable {
            return Err("structural delta observation cannot initialize writable access".into());
        }
        Kind::ObservedLocal(id)
    } else if let Kind::GlobalGet(entry) = value.kind
        && aggregate(&value.ty)
    {
        if !annotated {
            return Err("aggregate get requires a typed let or var binding".into());
        }
        Kind::BorrowedLocal(id, entry, mutable)
    } else if let Some((entry, source_mutable)) = provenance(value) {
        if source_mutable {
            return Err("cannot alias an exclusive writable global borrow".into());
        }
        if mutable {
            return Err("cannot upgrade a read-only global borrow to writable access".into());
        }
        Kind::BorrowedLocal(id, entry, false)
    } else if mutable {
        Kind::MutableLocal(id)
    } else {
        Kind::Local(id)
    };
    let mut binding = Value::new(value.ty.clone(), kind);
    binding.delta_required = value.delta_required;
    binding.snapshot = value.snapshot;
    Ok(binding)
}
/// Reject passing a borrowed aggregate through an ordinary helper boundary.
pub fn helper_argument(value: &Value) -> Result<(), String> {
    if value.snapshot && aggregate(&value.ty) {
        return Err("retained tuple aggregates cannot yet cross an ordinary helper or global replacement boundary".into());
    }
    if observed(value) {
        return Err(
            "structural delta observation cannot escape through an ordinary helper call".into(),
        );
    }
    if provenance(value).is_some() {
        return Err(
            "borrowed global aggregate cannot escape through an ordinary helper call".into(),
        );
    }
    Ok(())
}
/// Whether a structural delta is an evaluation-local input observation.
pub fn observed(value: &Value) -> bool {
    if !aggregate(&value.ty) {
        return false;
    }
    if let Kind::Field(parent, _) | Kind::Index(parent, _) = &value.kind {
        return observed(parent);
    }
    matches!(value.kind, Kind::ObservedLocal(_))
        || matches!(&value.kind, Kind::Query(name,args) if name == "delta_value" && (matches!(value.ty, Ty::Delta(_)) || args.first().is_some_and(|value| matches!(value.ty, Ty::Atomic(_)))))
}

/// Ordinary scalar projection; nominal arguments and delta origins remain exact.
pub fn project(ty: &Ty) -> Ty {
    if let Ty::Atomic(payload) = ty {
        return project(payload);
    }
    if let Ty::Struct(identity, fields, optional) = ty {
        return Ty::Struct(
            identity.clone(),
            fields
                .iter()
                .map(|(name, ty)| (name.clone(), project(ty)))
                .collect(),
            optional.clone(),
        );
    }
    if let Ty::List(child, size) = ty {
        return Ty::List(Box::new(project(child)), *size);
    }
    if let Ty::Tuple(children) = ty {
        return Ty::Tuple(children.iter().map(project).collect());
    }
    if let Ty::Map(key, child) = ty {
        return Ty::Map(Box::new(project(key)), Box::new(project(child)));
    }
    if let Ty::Set(child) = ty {
        return Ty::Set(Box::new(project(child)));
    }
    ty.clone()
}

pub use crate::static_values::StaticValues;

fn aggregate(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Family(_)
            | Ty::Set(_)
            | Ty::Map(..)
            | Ty::Recursive(_)
            | Ty::Struct(..)
            | Ty::List(..)
            | Ty::Tuple(_)
            | Ty::Delta(_)
    )
}
