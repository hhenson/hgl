//! Admission for complete ordinary Tuple publication and prepared child ownership.
use crate::ir::Value;
use hgl_source::Ty;
fn collection(ty: &Ty) -> bool {
    if matches!(ty, Ty::List(..) | Ty::Map(..)) {
        true
    } else if let Ty::Tuple(children) = ty {
        children.iter().any(collection)
    } else {
        false
    }
}
/// Admit prepared observation children while keeping native collection construction bounded.
pub fn constructor_child(value: &Value, constant_context: bool) -> Result<(), &'static str> {
    if !constant_context && !value.snapshot && (collection(&value.ty) || value.ty == Ty::Str) {
        return Err(
            "runtime ordinary tuple collection or text children require prepared retained observations",
        );
    }
    Ok(())
}

/// Resolve the ordinary tuple result boundary independently of an explicit delta requirement.
pub fn result(value: Value, result: &Ty, expected: &Ty, node: bool) -> Result<Value, String> {
    let value = crate::tuple_values::retain(value, false);
    if (value.ty != *expected && !(node && matches!(result, Ty::Tuple(_)) && value.ty == *result))
        || *result == Ty::Void
    {
        return Err("node return type mismatch".into());
    }
    if node && !value.snapshot && matches!(value.ty, Ty::Tuple(_)) && collection(&value.ty) {
        return Err(
            "runtime ordinary tuple collection children require prepared retained observations"
                .into(),
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

/// Variable-sized Tuple projections keep their prepared representation at helper boundaries.
pub fn value_arguments(args: &[Value]) -> Result<(), String> {
    if args.iter().any(|value| {
        value.ty == Ty::Str
            && (value.snapshot
                || (crate::tuple_values::endpoint(value)
                    && matches!(value.kind, crate::ir::Kind::Field(..))))
    }) {
        return Err(
            "retained text projections cannot yet cross an ordinary value-function boundary".into(),
        );
    }
    Ok(())
}
