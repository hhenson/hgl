//! Exact ordinary collection constructor checking, separate from sparse deltas.
use crate::ir::{Kind, Value};
use hgl_source::{Expr, Issue, Ty};
use std::collections::BTreeMap;
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
    let values = match (&ty, items.syntax()) {
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

/// Check the scalar-child collection effects and membership observation slice.
pub fn operation(name: &str, args: Vec<Value>) -> Result<Value, String> {
    let receiver = args
        .first()
        .ok_or("collection operation requires a receiver")?;
    let (key, child) = if let Ty::Map(key, child) = &receiver.ty {
        if **key != Ty::I64 {
            return Err("collection effects require i64 keys".into());
        }
        (key.as_ref(), child.as_ref())
    } else if let Ty::List(child, None) = &receiver.ty {
        (&Ty::I64, child.as_ref())
    } else {
        return Err("collection effects require an i64-key map or growing list".into());
    };
    if !(matches!(receiver.kind, Kind::Output)
        || name == "contains" && matches!(receiver.kind, Kind::Input(..)))
    {
        return Err("collection effect requires injected out".into());
    }
    if child.clone().delta()? != *child
        || matches!(child, Ty::List(..) | Ty::Tuple(_) | Ty::Struct(..))
    {
        return Err("collection effect requires a scalar child".into());
    }
    let map = matches!(receiver.ty, Ty::Map(..));
    let (size, payload) = match (map, name) {
        (true, "insert" | "update" | "upsert") => (3, true),
        (true, "remove" | "contains") | (_, "invalidate") => (2, false),
        (false, "push") => (2, true),
        (false, "pop") => (1, false),
        _ => return Err("unsupported collection operation".into()),
    };
    if args.len() != size
        || (size > 1
            && args[1].ty
                != if name == "push" {
                    child.clone()
                } else {
                    key.clone()
                })
        || (payload && args.last().is_some_and(|value| &value.ty != child))
    {
        return Err("collection operation argument type mismatch".into());
    }
    Ok(Value::new(
        if name == "contains" {
            Ty::Bool
        } else {
            Ty::Void
        },
        Kind::Query(format!("collection_{name}"), args),
    ))
}
type ItemsScope = (usize, usize, bool, Value, BTreeMap<String, Value>);
/// Fix exact key and child endpoint identities before checking a map iteration body.
pub fn items_scope(
    expr: &Expr,
    names: (&str, &str),
    env: &BTreeMap<String, Value>,
    next: &mut usize,
    mut check: impl FnMut(&Expr) -> Result<Value, Issue>,
) -> Result<ItemsScope, Issue> {
    let Expr::Call(operation, arguments) = expr.syntax() else {
        return Err("map iteration requires items(input, modified)".into());
    };
    if operation != "items"
        || !(1..=2).contains(&arguments.len())
        || arguments.get(1).is_some_and(|(_, expression)| !matches!(expression.syntax(), Expr::Name(name) if name == "modified"))
    {
        return Err("map iteration requires items(input, modified)".into());
    }
    let collection = check(&arguments[0].1)?;
    let (key, child) = if let Ty::Map(key, child) = &collection.ty {
        (*key.clone(), *child.clone())
    } else if let Ty::List(child, _) = &collection.ty {
        (Ty::I64, *child.clone())
    } else {
        return Err("items requires map or list input".into());
    };
    if !matches!(collection.kind, Kind::Input(..)) {
        return Err("items requires map input".into());
    }
    let key_id = *next;
    let child_id = key_id + 1;
    *next += 2;
    let mut scope = env.clone();
    scope.insert(names.0.into(), Value::new(key, Kind::Local(key_id)));
    scope.insert(
        names.1.into(),
        Value::new(child, Kind::IterationInput(child_id)),
    );
    Ok((key_id, child_id, arguments.len() == 2, collection, scope))
}
