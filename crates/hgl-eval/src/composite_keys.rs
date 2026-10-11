//! Complete finite key identities for cold validation, never runtime dispatch.
use crate::ir::{Kind, Value};
use crate::value_eval::EvalError;
use hgl_value_model::{Literal, Ty};
/// Exact scalar or complete composite identity, including each optional presence bit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// Existing exact scalar identity, including nominal enums and signed zero.
    Scalar(crate::scalar_keys::Key),
    /// Provider-equal native identity interned within one cold evaluation.
    Native(usize),
    /// A canonical owning box identity, with its actual contained capabilities validated.
    Boxed(Option<Box<Self>>),
    /// Complete ordered or canonically sorted unordered contents.
    Sequence(String, Vec<Self>),
    /// Exact tuple/struct type and declaration-ordered independently owned components.
    Fields(Ty, Vec<Option<Self>>),
}
/// Scalar identities also serve fixed structural child indices in cold trace state.
pub fn scalar(value: &Literal) -> Result<Key, String> {
    crate::scalar_keys::key(value).map(Key::Scalar)
}
/// Require complete materialization before duplicate or membership validation.
pub fn key(value: &Value) -> Result<Key, String> {
    known(value)?.ok_or_else(|| "collection key requires cold materialization".into())
}
/// Return no identity while any leaf still needs a contextual provider.
pub fn known(value: &Value) -> Result<Option<Key>, String> {
    known_with(value, &mut |_| Ok(None)).map_err(|e| e.to_string())
}
pub(crate) fn known_with(
    value: &Value,
    native: &mut impl FnMut(&Value) -> Result<Option<Key>, EvalError>,
) -> Result<Option<Key>, EvalError> {
    if value.ty == Ty::Any {
        return boxed(value, native);
    }
    if !value.ty.collection_key() {
        return Err("unsupported composite collection key component".into());
    }
    if matches!(value.kind, Kind::Native(..)) && value.cold_call {
        return Ok(None);
    }
    if matches!(value.kind, Kind::Opaque(_)) {
        return native(value);
    }
    if let Kind::Literal(literal) = &value.kind
        && literal.ty() == value.ty
    {
        return scalar(literal).map(Some).map_err(EvalError::from);
    }
    if let Kind::TemporalLiteral(recipe) = &value.kind
        && recipe.ty() == value.ty
    {
        return Ok(None);
    }
    let Kind::Construct(values) = &value.kind else {
        return Err("complete constant collection key required".into());
    };
    let (fields, optional) = crate::any_values::fields(&value.ty)
        .ok_or("complete tuple or concrete struct key required")?;
    let mut components = vec![None; fields.len()];
    let mut seen = vec![false; fields.len()];
    let mut deferred = false;
    for (index, child) in values {
        if fields.get(*index).is_none_or(|ty| *ty != child.ty) {
            return Err("composite key field type or position mismatch".into());
        }
        if std::mem::replace(&mut seen[*index], true) {
            return Err("duplicate composite key field".into());
        }
        components[*index] = known_with(child, native)?;
        deferred |= components[*index].is_none();
    }
    if seen
        .iter()
        .enumerate()
        .any(|(i, present)| !present && !optional.contains(&i))
    {
        return Err("composite key requires every required field".into());
    }
    Ok((!deferred).then(|| Key::Fields(value.ty.clone(), components)))
}

fn boxed(
    value: &Value,
    native: &mut impl FnMut(&Value) -> Result<Option<Key>, EvalError>,
) -> Result<Option<Key>, EvalError> {
    let Some(value) = crate::any_values::contents(value) else {
        return Ok(Some(Key::Boxed(None)));
    };
    if !crate::any_values::capable(&value.ty, 0) || !crate::any_values::capable(&value.ty, 1) {
        return Err(crate::any_values::capability());
    }
    complete(value, native).map(|value| value.map(|value| Key::Boxed(Some(Box::new(value)))))
}
fn complete(
    value: &Value,
    native: &mut impl FnMut(&Value) -> Result<Option<Key>, EvalError>,
) -> Result<Option<Key>, EvalError> {
    let value = crate::family_values::concrete(value);
    if value.ty.collection_key() {
        return known_with(value, native);
    }
    if let Kind::TemporalLiteral(_) = &value.kind {
        return Ok(None);
    }
    if let Kind::List(items) = &value.kind {
        let values = items
            .iter()
            .map(|value| complete(value, native))
            .collect::<Result<Option<Vec<_>>, _>>()?;
        return Ok(values.map(|mut values| {
            if matches!(value.ty, Ty::Set(_) | Ty::Map(..)) {
                values.sort();
            }
            Key::Sequence(value.ty.source_name(), values)
        }));
    }
    if let Kind::Construct(fields) = &value.kind {
        let (_, declared, _) = value.ty.structure()?;
        let mut values = vec![None; declared.len()];
        for (id, child) in fields {
            let Some(child) = complete(child, native)? else {
                return Ok(None);
            };
            values[*id] = Some(child);
        }
        return Ok(Some(Key::Fields(value.ty.clone(), values)));
    }
    Err("complete owning boxed key required".into())
}
