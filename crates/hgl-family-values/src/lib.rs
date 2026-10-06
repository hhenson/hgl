//! Checked ordinary family widening and concrete-tag-preserving cold ownership.
use hgl_rust_ir::{Kind, Value};
use hgl_source::Ty;
/// Admit only a concrete declared member or an explicit subfamily ancestor edge.
pub fn coerce(expected: Option<&Ty>, value: Value) -> Result<Value, String> {
    let Some(Ty::Family(family)) = expected else {
        return Ok(value);
    };
    if expected == Some(&value.ty) {
        return Ok(value);
    }
    let admitted = if let Ty::Family(source) = &value.ty {
        source.ancestors().contains(family.identity())
    } else {
        family.members().iter().any(|(_, ty)| ty == &value.ty)
    };
    if !admitted {
        return Err("value is not an admitted concrete member or subfamily".into());
    }
    Ok(Value::new(
        Ty::Family(family.clone()),
        Kind::Unary("family".into(), Box::new(value)),
    ))
}
/// Build the canonical owning member wrapper after evaluating its operand once.
pub fn retain(target: &Ty, value: Value) -> Result<Value, String> {
    let Ty::Family(family) = target else {
        return Err("family target required".into());
    };
    let value = if let Ty::Family(source) = &value.ty {
        let Kind::Construct(mut fields) = value.kind else {
            return Err("family member value required".into());
        };
        if fields.len() != 1 {
            return Err("family requires exactly one concrete member".into());
        }
        let (index, member) = fields.remove(0);
        if source
            .members()
            .get(index)
            .is_none_or(|(_, ty)| *ty != member.ty)
        {
            return Err("concrete family tag does not match its payload".into());
        }
        member
    } else {
        value
    };
    let index = family
        .members()
        .iter()
        .position(|(_, ty)| ty == &value.ty)
        .ok_or("concrete member is outside declared family")?;
    Ok(Value::new(
        target.clone(),
        Kind::Construct(vec![(index, value)]),
    ))
}
/// Inspect a canonical wrapper without losing its concrete member's exact type.
pub fn concrete(value: &Value) -> &Value {
    if matches!(value.ty, Ty::Family(_))
        && let Kind::Construct(fields) = &value.kind
        && fields.len() == 1
    {
        return &fields[0].1;
    }
    value
}

/// Compare cold owning descendants after checking exact concrete nominal tags.
pub fn equal(a: &Value, b: &Value) -> bool {
    let (a, b) = (concrete(a), concrete(b));
    if a.ty != b.ty {
        return false;
    }
    match (&a.kind, &b.kind) {
        (Kind::Literal(a), Kind::Literal(b)) => a == b,
        (Kind::List(a), Kind::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Kind::Construct(a), Kind::Construct(b)) => {
            a.len() == b.len()
                && a.iter().all(|(id, a)| {
                    b.iter()
                        .find(|(i, _)| i == id)
                        .is_some_and(|(_, b)| equal(a, b))
                })
        }
        _ => false,
    }
}
