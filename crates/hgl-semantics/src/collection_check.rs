//! Exact ordinary collection constructor checking, separate from sparse deltas.
use crate::ir::{Kind, Value};
use hgl_source::{Expr, Issue, Ty};
/// Check the sole named items argument with exact key and payload expectations.
pub fn constructor(
    ty: &Ty,
    args: &[(Option<String>, Expr)],
    mut check: impl FnMut(&Expr, &Ty) -> Result<Value, String>,
) -> Result<Value, String> {
    constructor_checked(ty, args, |expr, ty| check(expr, ty).map_err(Issue::from))
        .map_err(String::from)
}
/// Check collection entries without discarding the caller's source error identity.
pub fn constructor_checked(
    ty: &Ty,
    args: &[(Option<String>, Expr)],
    mut check: impl FnMut(&Expr, &Ty) -> Result<Value, Issue>,
) -> Result<Value, Issue> {
    if !crate::value_access::ordinary(ty) {
        return Err("unsupported ordinary collection payload".into());
    }
    let ty = crate::value_access::project(ty);
    if !ty.atomic_payload() {
        return Err("unsupported ordinary collection payload".into());
    }
    let [(Some(name), items)] = args else {
        return Err("collection constructor requires items".into());
    };
    if name != "items" {
        return Err("collection constructor requires items".into());
    }
    let mut known = std::collections::BTreeSet::new();
    let mut key =
        |expr: &Expr, ty: &Ty, check: &mut dyn FnMut(&Expr, &Ty) -> Result<Value, Issue>| {
            let value = check(expr, ty)?;
            if value.ty != *ty {
                return Err("ordinary collection key type mismatch".into());
            }
            if crate::value_constant::context_free(&value) {
                let closed = crate::value_eval::Evaluator::default()
                    .value(&value)
                    .map_err(|e| e.to_string())?;
                if !known.insert(crate::collection_values::key(&closed)?) {
                    return Err("duplicate ordinary collection key or member".into());
                }
            }
            Ok(value)
        };
    let values = match (&ty, items) {
        (Ty::Set(member), Expr::Sequence(items)) => items
            .iter()
            .map(|expr| {
                key(
                    expr.as_ref().ok_or("set member cannot be silent")?,
                    member,
                    &mut check,
                )
            })
            .collect::<Result<Vec<_>, Issue>>()?,
        (Ty::Map(k, v), Expr::Sparse(items)) => items
            .iter()
            .map(|(a, b)| {
                let a = key(a, k, &mut check)?;
                let b = check(b, v)?;
                if b.ty != **v {
                    return Err("ordinary map value type mismatch".into());
                }
                Ok(Value::new(
                    Ty::Tuple(vec![(**k).clone(), (**v).clone()]),
                    Kind::Construct(vec![(0, a), (1, b)]),
                ))
            })
            .collect::<Result<Vec<_>, Issue>>()?,
        (Ty::Map(..), Expr::Sequence(items)) if items.is_empty() => Vec::new(),
        _ => return Err("ordinary collection items have the wrong entry form".into()),
    };
    Ok(Value::new(ty, Kind::List(values)))
}
