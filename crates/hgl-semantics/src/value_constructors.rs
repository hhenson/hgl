//! Checked explicit ordinary constructor boundaries preserve evaluation phases.
/// Resolve explicit built-in constructors without performing optional execution.
pub fn constructor(
    name: &str,
    args: &[(Option<String>, hgl_source::Expr)],
    mut check: impl FnMut(
        &hgl_source::Expr,
        Option<&hgl_source::Ty>,
    ) -> Result<crate::ir::Value, hgl_source::Issue>,
) -> Result<crate::ir::Value, hgl_source::Issue> {
    if name == "bytes" {
        bytes(args, |expr, hint| check(expr, Some(hint)))
    } else {
        crate::any_values::checked(args, |expr| check(expr, None))
    }
}

/// Check the ordinary constructor without moving execution to checking time.
pub fn bytes(
    args: &[(Option<String>, hgl_source::Expr)],
    mut check: impl FnMut(
        &hgl_source::Expr,
        &hgl_source::Ty,
    ) -> Result<crate::ir::Value, hgl_source::Issue>,
) -> Result<crate::ir::Value, hgl_source::Issue> {
    use crate::ir::{Kind, Value};
    use hgl_source::Ty;
    if args.len() > 1 || args.iter().any(|(label, _)| label.is_some()) {
        return Err("bytes requires zero or one positional argument".into());
    }
    let list = Ty::List(Box::new(Ty::I64), None);
    let argument = args
        .first()
        .map(|(_, expr)| check(expr, &list))
        .transpose()?
        .unwrap_or_else(|| Value::new(list, Kind::List(Vec::new())));
    if !matches!(
        (&argument.ty, &argument.kind),
        (Ty::Atomic(_), Kind::Input(_, false))
    ) {
        crate::endpoint_check::require_payload(&argument)?;
    }
    if !matches!(&argument.ty, Ty::List(element, _) if **element == Ty::I64)
        && !matches!(&argument.ty, Ty::Atomic(payload) if matches!(payload.as_ref(), Ty::List(element, _) if **element == Ty::I64))
    {
        return Err("bytes requires one ordinary i64 list".into());
    }
    Ok(Value::new(
        Ty::Bytes,
        Kind::Unary("bytes".into(), Box::new(argument)),
    ))
}
