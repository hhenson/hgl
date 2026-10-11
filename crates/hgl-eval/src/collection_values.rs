//! Ordered construction and unordered identity of complete ordinary collections.
/// Validate a complete retained key, preserving optional presence and exact types.
pub use crate::composite_keys::key;
use crate::ir::{Kind, Value};
use hgl_value_model::Ty;
/// Retain each key before checking duplicates, and each map value only afterwards.
pub fn evaluate<E: From<String> + From<crate::value_eval::EvalError>>(
    ty: &Ty,
    items: &[Value],
    mut evaluate: impl FnMut(&Value) -> Result<Value, E>,
) -> Result<Vec<Value>, E> {
    let mut keys = std::collections::BTreeSet::new();
    let mut identities = crate::key_context::Context::default();
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
        crate::any_values::validate_key(&retained).map_err(E::from)?;
        if !keys.insert(identities.key(&retained).map_err(E::from)?) {
            return Err(E::from(
                "duplicate ordinary collection key or member".to_owned(),
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
/// Compare complete ordinary values through fallible provider and boxed operations.
pub use crate::any_values::equal_data as equal;
