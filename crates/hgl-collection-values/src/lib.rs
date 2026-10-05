//! Ordered construction and unordered identity of complete ordinary collections.
use hgl_rust_ir::{Kind, Value};
use hgl_source::Ty;
/// Validate one retained scalar key, including the non-NaN boundary.
pub fn key(value: &Value) -> Result<hgl_scalar_keys::Key, String> {
    let Kind::Literal(value) = &value.kind else {
        return Err("ordinary collection requires a scalar key".into());
    };
    hgl_scalar_keys::key(value)
}
/// Retain each key before checking duplicates, and each map value only afterwards.
pub fn evaluate<E: From<String>>(
    ty: &Ty,
    items: &[Value],
    mut evaluate: impl FnMut(&Value) -> Result<Value, E>,
) -> Result<Vec<Value>, E> {
    let mut keys = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        let (source, child) = if matches!(ty, Ty::Map(..)) {
            let Kind::Construct(fields) = &item.kind else {
                unreachable!("checked map entry")
            };
            (&fields[0].1, Some(&fields[1].1))
        } else {
            (item, None)
        };
        let retained = evaluate(source)?;
        if !keys.insert(key(&retained).map_err(E::from)?) {
            return Err(E::from(
                "duplicate ordinary collection key or member".into(),
            ));
        }
        result.push(if let Some(child) = child {
            Value::new(
                item.ty.clone(),
                Kind::Construct(vec![(0, retained), (1, evaluate(child)?)]),
            )
        } else {
            retained
        });
    }
    Ok(result)
}
/// Compare recursively retained ordinary data, ignoring complete collection order.
pub fn equal(a: &Value, b: &Value) -> bool {
    let (a, b) = (
        hgl_family_values::concrete(a),
        hgl_family_values::concrete(b),
    );
    if a.ty != b.ty {
        return false;
    }
    match (&a.kind, &b.kind) {
        (Kind::Literal(a), Kind::Literal(b)) => a == b,
        (Kind::Void, Kind::Void) => true,
        (Kind::List(left), Kind::List(right)) => {
            left.len() == right.len()
                && if matches!(a.ty, Ty::Set(_) | Ty::Map(..)) {
                    left.iter().all(|a| right.iter().any(|b| equal(a, b)))
                } else {
                    left.iter().zip(right).all(|(a, b)| equal(a, b))
                }
        }
        (Kind::Construct(a), Kind::Construct(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(id, a)| b.iter().any(|(other, b)| id == other && equal(a, b)))
        }
        _ => false,
    }
}
