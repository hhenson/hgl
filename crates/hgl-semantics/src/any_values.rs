//! Source admission of owning boxes above checked value execution.
use crate::ir::{Kind, Value};
pub use hgl_eval::any_values::*;
use hgl_source::{Expr, Issue, Ty};

/// Check explicit boxing while preserving the ordinary argument's evaluation phase.
pub fn checked(
    args: &[(Option<String>, Expr)],
    mut check: impl FnMut(&Expr) -> Result<Value, Issue>,
) -> Result<Value, Issue> {
    if args.len() > 1 || args.iter().any(|(label, _)| label.is_some()) {
        return Err("any requires zero or one positional argument".into());
    }
    let Some((_, argument)) = args.first() else {
        return Ok(Value::new(Ty::Any, Kind::Construct(Vec::new())));
    };
    let mut value = check(argument)?;
    if let Ty::Atomic(payload) = &value.ty {
        let payload = *payload.clone();
        value = crate::value_eval::atomic_argument(value, Some(&payload));
    }
    crate::endpoint_check::require_payload(&value)?;
    if !crate::value_access::ordinary(&value.ty)
        || matches!(value.kind, Kind::Wire(_) | Kind::Capability | Kind::Output)
    {
        return Err("any requires an admitted ordinary owning value".into());
    }
    Ok(Value::new(
        Ty::Any,
        Kind::Unary("any".into(), Box::new(value)),
    ))
}
