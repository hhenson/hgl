//! Ordinary owning boxes retain data and concrete identity, never view authority.
use crate::ir::{DeltaEntry, Kind, Value};
use crate::value_eval::EvalError;
pub use hgl_value_model::ordinary_shape::{capable, contains_any, fields};
use hgl_value_model::{Literal, Ty};
use std::cmp::Ordering;

/// Flatten existing boxes and concrete family members after evaluating the argument once.
pub fn retain(value: Value) -> Value {
    if value.ty == Ty::Any {
        return value;
    }
    let value = crate::family_values::concrete(&value).clone();
    Value::new(Ty::Any, Kind::Construct(vec![(0, value)]))
}
/// The retained payload, with nil represented by an empty owning box.
pub fn contents(value: &Value) -> Option<&Value> {
    let Kind::Construct(fields) = &value.kind else {
        unreachable!("closed owning box");
    };
    fields.first().map(|(_, value)| value)
}
/// Missing contained capabilities remain executed failures with their stable identifier.
pub fn capability() -> EvalError {
    EvalError::Coded(
        "value.capability",
        "contained value lacks the required capability".into(),
    )
}
/// Compare whole boxes before requesting any contained operation.
pub fn binary(op: &str, a: &Value, b: &Value) -> Result<Value, EvalError> {
    let (a, b) = (contents(a), contents(b));
    let equality = matches!(op, "==" | "!=");
    let result = match (a, b) {
        (None, None) => matches!(op, "==" | "<=" | ">="),
        (None, Some(_)) => matches!(op, "!=" | "<" | "<="),
        (Some(_), None) => matches!(op, "!=" | ">" | ">="),
        (Some(a), Some(b)) if a.ty.source_name() != b.ty.source_name() => op == "!=",
        (Some(a), Some(b)) => {
            if !capable(&a.ty, if equality { 0 } else { 2 }) {
                return Err(capability());
            }
            if equality {
                equal_data(a, b)? == (op == "==")
            } else {
                let order = order(a, b)?;
                match op {
                    "<" => order == Some(Ordering::Less),
                    "<=" => matches!(order, Some(Ordering::Less | Ordering::Equal)),
                    ">" => order == Some(Ordering::Greater),
                    ">=" => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
                    _ => unreachable!("checked any comparison"),
                }
            }
        }
    };
    Ok(Value::new(Ty::Bool, Kind::Literal(Literal::Bool(result))))
}
/// Compare retained ordinary/harness data, propagating nested box capability failures.
pub fn equal_data(a: &Value, b: &Value) -> Result<bool, EvalError> {
    if a.ty == Ty::Error {
        return equal_error(a, b);
    }

    let (a, b) = (
        crate::family_values::concrete(a),
        crate::family_values::concrete(b),
    );
    if a.ty.source_name() != b.ty.source_name() {
        return Ok(false);
    }
    if a.ty == Ty::Any {
        let value = binary("==", a, b)?;
        let Kind::Literal(Literal::Bool(equal)) = value.kind else {
            unreachable!()
        };
        return Ok(equal);
    }
    match (&a.kind, &b.kind) {
        (Kind::Opaque(left), Kind::Opaque(right)) => {
            if !capable(&a.ty, 0) {
                return Err(capability());
            }
            Ok((left.operations().equal)(left.data(), right.data())?)
        }
        (Kind::Literal(a), Kind::Literal(b)) => Ok(a == b),
        (Kind::Void, Kind::Void) => Ok(true),
        (Kind::List(left), Kind::List(right)) => {
            sequence_equal(left, right, matches!(a.ty, Ty::Set(_) | Ty::Map(..)))
        }
        (Kind::Construct(left), Kind::Construct(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (id, left) in left {
                let Some((_, right)) = right.iter().find(|(other, _)| id == other) else {
                    return Ok(false);
                };
                if !equal_data(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Kind::Delta(left), Kind::Delta(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for left in left {
                let mut found = false;
                for right in right {
                    let equal = match (left, right) {
                        (DeltaEntry::Add(a), DeltaEntry::Add(b))
                        | (DeltaEntry::Remove(a), DeltaEntry::Remove(b)) => equal_data(a, b)?,
                        (DeltaEntry::Keyed(i, a), DeltaEntry::Keyed(j, b)) => {
                            equal_data(i, j)? && equal_data(a, b)?
                        }
                        (DeltaEntry::Child(i, a), DeltaEntry::Child(j, b)) => {
                            i == j && equal_data(a, b)?
                        }
                        _ => false,
                    };
                    if equal {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
fn sequence_equal(left: &[Value], right: &[Value], unordered: bool) -> Result<bool, EvalError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    if unordered {
        for left in left {
            if !right
                .iter()
                .try_fold(false, |found, right| -> Result<bool, EvalError> {
                    Ok(found || equal_data(left, right)?)
                })?
            {
                return Ok(false);
            }
        }
    } else {
        for (left, right) in left.iter().zip(right) {
            if !equal_data(left, right)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
fn order(a: &Value, b: &Value) -> Result<Option<Ordering>, EvalError> {
    let (a, b) = (
        crate::family_values::concrete(a),
        crate::family_values::concrete(b),
    );
    if a.ty.source_name() != b.ty.source_name() {
        return Ok(None);
    }
    if a.ty == Ty::Any {
        for (op, ordering) in [
            ("<", Ordering::Less),
            (">", Ordering::Greater),
            ("==", Ordering::Equal),
        ] {
            if matches!(binary(op, a, b)?.kind, Kind::Literal(Literal::Bool(true))) {
                return Ok(Some(ordering));
            }
        }
        return Ok(None);
    }
    match (&a.kind, &b.kind) {
        (Kind::Opaque(left), Kind::Opaque(right)) => {
            if !capable(&a.ty, 2) {
                return Err(capability());
            }
            Ok((left.operations().compare)(left.data(), right.data())?)
        }
        (Kind::Literal(a), Kind::Literal(b)) => Ok(match (a, b) {
            (Literal::Float(a), Literal::Float(b)) => a.partial_cmp(b),
            (Literal::Str(a), Literal::Str(b)) => Some(a.cmp(b)),
            (Literal::Bytes(a), Literal::Bytes(b)) => Some(a.cmp(b)),
            _ => Some(
                crate::scalar_keys::key(a)
                    .map_err(EvalError::from)?
                    .cmp(&crate::scalar_keys::key(b).map_err(EvalError::from)?),
            ),
        }),
        (Kind::List(left), Kind::List(right)) => {
            for (left, right) in left.iter().zip(right) {
                let ordered = order(left, right)?;
                if ordered != Some(Ordering::Equal) {
                    return Ok(ordered);
                }
            }
            Ok(Some(left.len().cmp(&right.len())))
        }
        (Kind::Construct(left), Kind::Construct(right)) => {
            let (fields, _) = fields(&a.ty).ok_or_else(capability)?;
            for id in 0..fields.len() {
                let (left, right) = (
                    left.iter().find(|(index, _)| *index == id),
                    right.iter().find(|(index, _)| *index == id),
                );
                let ordered = match (left, right) {
                    (None, None) => Some(Ordering::Equal),
                    (None, Some(_)) => Some(Ordering::Less),
                    (Some(_), None) => Some(Ordering::Greater),
                    (Some((_, a)), Some((_, b))) => order(a, b)?,
                };
                if ordered != Some(Ordering::Equal) {
                    return Ok(ordered);
                }
            }
            Ok(Some(Ordering::Equal))
        }
        _ => Err(capability()),
    }
}

/// Validate each actual boxed key before membership or duplicate comparison can short circuit.
pub fn validate_key(value: &Value) -> Result<(), EvalError> {
    if value.ty == Ty::Any {
        if let Some(payload) = contents(value) {
            if !capable(&payload.ty, 0) || !capable(&payload.ty, 1) {
                return Err(capability());
            }
            validate_key(payload)?;
        }
    } else if let Kind::List(items) = &value.kind {
        for item in items {
            validate_key(item)?;
        }
    } else if let Kind::Construct(fields) = &value.kind {
        for (_, field) in fields {
            validate_key(field)?;
        }
    }
    Ok(())
}

fn equal_error(a: &Value, b: &Value) -> Result<bool, EvalError> {
    let (Kind::Construct(a), Kind::Construct(b)) = (&a.kind, &b.kind) else {
        return Err(crate::value_operations::unsupported(
            "error equality requires diagnostic data",
        ));
    };
    for index in [0, 1] {
        match (
            a.iter().find(|(id, _)| *id == index),
            b.iter().find(|(id, _)| *id == index),
        ) {
            (Some((_, a)), Some((_, b))) if equal_data(a, b)? => {}
            (None, None) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}
