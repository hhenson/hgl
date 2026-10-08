//! Ordinary tuple projection and retained temporal-value representation.
use crate::ir::{Kind, Value};
use hgl_source::{Literal, Ty};
/// Whether this endpoint belongs to a statically projected tuple input.
pub fn endpoint(value: &Value) -> bool {
    if let Kind::Field(parent, _) = &value.kind {
        endpoint(parent)
    } else {
        matches!(value.kind, Kind::Input(..) | Kind::IterationInput(_))
    }
}
/// Recursively supported retained tuple observations, without nominal publication changes.
pub fn supported(ty: &Ty) -> bool {
    profile(ty, true)
}
fn profile(ty: &Ty, growing: bool) -> bool {
    if let Ty::Tuple(children) = ty {
        children.iter().all(|child| profile(child, growing))
    } else if let Ty::Struct(_, fields, _) = ty {
        fields.iter().all(|(_, child)| profile(child, false))
    } else if let Ty::List(child, size) = ty {
        (growing || size.is_some()) && profile(child, growing)
    } else if let Ty::Map(key, child) = ty {
        key.collection_key()
            && !matches!(key.as_ref(), Ty::Tuple(_) | Ty::Struct(..) | Ty::Enum(_))
            && profile(child, growing)
    } else {
        ty.atomic_payload()
            && !matches!(
                ty,
                Ty::Recursive(_) | Ty::Family(_) | Ty::Enum(_) | Ty::Set(_)
            )
    }
}
/// Retain a current tuple value; source identity and child validity remain exact.
pub fn retain(value: Value, text_child: bool) -> Value {
    if (ordinary_result(&value.ty)
        || ((text_child || matches!(value.kind, Kind::Field(..))) && value.ty == Ty::Str))
        && profile(&value.ty, matches!(value.ty, Ty::Tuple(_)))
        && endpoint(&value)
        && !matches!(value.kind, Kind::Input(_, true))
    {
        let mut retained = Value::new(
            value.ty.clone(),
            Kind::Unary("snapshot".into(), Box::new(value)),
        );
        retained.snapshot = true;
        retained
    } else {
        value
    }
}
/// Resolve a tuple's constant position and preserve representation provenance.
pub fn indexed(parent: Value, index: Value) -> Result<Value, String> {
    if matches!(parent.kind, Kind::Input(_, true)) {
        return Err("signal has no ordinary scalar value".into());
    }
    if let Ty::List(element, _) = &parent.ty {
        if endpoint(&parent) || matches!(parent.kind, Kind::Wire(_) | Kind::Output) {
            return Err(
                "temporal child indexing is outside the admitted publication-delta profile".into(),
            );
        }
        if index.ty != Ty::I64 {
            return Err("ordinary list index requires i64".into());
        }
        let mut result = Value::new(
            *element.clone(),
            Kind::Index(Box::new(parent.clone()), Box::new(index)),
        );
        result.snapshot = parent.snapshot;
        return Ok(result);
    }
    let Kind::Literal(Literal::Int(position)) = index.kind else {
        return Err("tuple index requires a constant integer position".into());
    };
    let Ty::Tuple(children) = &parent.ty else {
        return Err("indexing requires an ordinary list or tuple".into());
    };
    let position = usize::try_from(position).map_err(|_error| "tuple index out of bounds")?;
    let ty = children
        .get(position)
        .ok_or("tuple index out of bounds")?
        .clone();
    if matches!(parent.kind, Kind::Wire(_) | Kind::Output) {
        return Err("tuple indexing requires an ordinary value or runtime input".into());
    }
    let snapshot = parent.snapshot;
    let mut value = Value::new(ty, Kind::Field(Box::new(parent), position));
    value.snapshot = snapshot;
    Ok(value)
}

/// Implicit ordinary tuple output accepts complete values; explicit delta requirements stay exact.
pub fn delta_result(explicit: bool, result: &Ty, expected: &Ty) -> bool {
    explicit || (matches!(expected, Ty::Delta(_)) && !ordinary_result(result))
}
/// Ordinary complete structural publication remains separate from explicit delta requirements.
pub fn ordinary_result(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Tuple(_) | Ty::Struct(..) | Ty::Map(..) | Ty::List(..)
    )
}

/// Whether tuple construction includes an owning aggregate child.
pub fn nested_snapshot(values: &[Value]) -> bool {
    values.iter().any(|v| v.snapshot)
}
