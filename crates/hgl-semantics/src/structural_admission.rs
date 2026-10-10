//! Complete ordinary structural return and own-output source admission.
use crate::ir::{Kind, Statement, Value};
use hgl_source::Ty;
/// Whether a native ordinary aggregate contains collection storage.
pub fn collection(ty: &Ty) -> bool {
    if matches!(ty, Ty::List(..) | Ty::Map(..)) {
        true
    } else if let Ty::Tuple(children) = ty {
        children.iter().any(collection)
    } else if let Ty::Struct(_, fields, _) = ty {
        fields.iter().any(|(_, child)| collection(child))
    } else {
        false
    }
}
/// Resolve the ordinary structural result boundary independently of an explicit delta requirement.
pub fn result(value: Value, result: &Ty, expected: &Ty, node: bool) -> Result<Value, String> {
    let value = crate::tuple_values::retain(value, false);
    if node
        && !matches!(result, Ty::Atomic(_) | Ty::Rolling(..))
        && crate::tuple_values::ordinary_result(&value.ty)
        && (!nonempty(&value.ty)
            || matches!(&value.kind,Kind::Construct(fields) if fields.is_empty()))
    {
        return Err(
            "complete ordinary publication requires a nonempty value retaining a valid child"
                .into(),
        );
    }
    if (value.ty != *expected
        && !(node && crate::tuple_values::ordinary_result(result) && value.ty == *result))
        || *result == Ty::Void
    {
        return Err("node return type mismatch".into());
    }
    if node
        && (!matches!(result, Ty::Atomic(_) | Ty::Rolling(..))
            || matches!(
                value.kind,
                Kind::Construct(_) | Kind::Local(_) | Kind::MutableLocal(_)
            ))
        && (crate::tuple_values::ordinary_result(&value.ty)
            || (crate::tuple_values::endpoint(&value) && matches!(value.kind, Kind::Field(..))))
        && !value.snapshot
        && (collection(&value.ty) || variable(&value.ty))
    {
        return Err(
            "runtime ordinary collection children require prepared retained observations".into(),
        );
    }
    if value.snapshot && matches!(result, Ty::Atomic(_)) {
        return Err("retained Tuple observations cannot cross an atomic result boundary".into());
    }
    crate::endpoint_check::require_payload(&value)?;
    Ok(value)
}

/// Preserve the ordinary node delta context without applying it to value functions.
pub fn expected(result: &Ty, node: bool) -> Result<Ty, hgl_source::Issue> {
    if node && !matches!(result, Ty::Ref(_)) {
        result.clone().delta()
    } else {
        Ok(result.clone())
    }
}

fn variable(ty: &Ty) -> bool {
    if matches!(
        ty,
        Ty::Set(_) | Ty::Str | Ty::Bytes | Ty::TimeZone | Ty::ZonedTime | Ty::ZonedDateTime
    ) {
        true
    } else if let Ty::Atomic(child) = ty {
        collection(child) || variable(child)
    } else if let Ty::Tuple(children) = ty {
        children.iter().any(variable)
    } else if let Ty::Struct(_, fields, _) = ty {
        fields.iter().any(|(_, child)| variable(child))
    } else {
        false
    }
}
fn nonempty(ty: &Ty) -> bool {
    if let Ty::Tuple(children) = ty {
        !children.is_empty() && children.iter().all(nonempty)
    } else if let Ty::Struct(_, fields, _) = ty {
        !fields.is_empty() && fields.iter().all(|(_, child)| nonempty(child))
    } else if let Ty::List(child, _) | Ty::Map(_, child) = ty {
        nonempty(child)
    } else {
        true
    }
}
/// Own output assignments use the same complete-value/delta boundary as returns.
pub fn assignment(target: Value, value: Value, expected: &Ty) -> Result<Statement, String> {
    let value = result(value, &target.ty, expected, true).map_err(|error| {
        if error == "node return type mismatch" {
            "assignment type mismatch".into()
        } else {
            error
        }
    })?;
    Ok(Statement::Assign(target, value))
}
