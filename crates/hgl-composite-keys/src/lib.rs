//! Complete finite key identities for cold validation, never runtime dispatch.
use hgl_rust_ir::{Kind, Value};
use hgl_source::{Literal, Ty};
/// Exact scalar or complete composite identity, including each optional presence bit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// Existing exact scalar identity, including nominal enums and signed zero.
    Scalar(hgl_scalar_keys::Key),
    /// Exact tuple/struct type and declaration-ordered independently owned components.
    Fields(Ty, Vec<Option<Self>>),
}
/// Scalar identities also serve fixed structural child indices in cold trace state.
pub fn scalar(value: &Literal) -> Result<Key, String> {
    hgl_scalar_keys::key(value).map(Key::Scalar)
}
/// Require complete materialization before duplicate or membership validation.
pub fn key(value: &Value) -> Result<Key, String> {
    known(value)?.ok_or_else(|| "collection key requires cold materialization".into())
}
/// Return no identity while any leaf still needs a contextual provider.
pub fn known(value: &Value) -> Result<Option<Key>, String> {
    if !value.ty.collection_key() {
        return Err("unsupported composite collection key component".into());
    }
    if let Kind::Literal(literal) = &value.kind
        && literal.ty() == value.ty
    {
        return scalar(literal).map(Some);
    }
    if let Kind::TemporalLiteral(recipe) = &value.kind
        && recipe.ty() == value.ty
    {
        return Ok(None);
    }
    let Kind::Construct(values) = &value.kind else {
        return Err("complete constant collection key required".into());
    };
    let (fields, optional): (Vec<_>, &[usize]) = if let Ty::Tuple(fields) = &value.ty {
        (fields.iter().collect(), &[])
    } else if let Ty::Struct(_, fields, optional) = &value.ty {
        (fields.iter().map(|(_, ty)| ty).collect(), optional)
    } else {
        return Err("complete tuple or concrete struct key required".into());
    };
    let mut components = vec![None; fields.len()];
    let mut seen = vec![false; fields.len()];
    let mut deferred = false;
    for (index, child) in values {
        if fields.get(*index).is_none_or(|ty| **ty != child.ty) {
            return Err("composite key field type or position mismatch".into());
        }
        if std::mem::replace(&mut seen[*index], true) {
            return Err("duplicate composite key field".into());
        }
        components[*index] = known(child)?;
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
