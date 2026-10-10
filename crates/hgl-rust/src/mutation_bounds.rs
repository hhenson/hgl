//! Cold finite membership bounds without executing user expressions.
use hgl_semantics::ir::{Kind, Statement, Value};
use hgl_source::Literal;
use std::collections::BTreeMap;
fn membership(op: &str) -> bool {
    matches!(
        op,
        "set_upsert" | "set_discard" | "set_insert" | "set_remove"
    ) || (op.starts_with("collection_") && op != "collection_contains")
}
type Constants = BTreeMap<usize, i64>;
fn integer(value: &Value, constants: &Constants) -> Option<i64> {
    if let Kind::Literal(Literal::Int(value)) = &value.kind {
        return Some(*value);
    }
    if let Kind::Local(id) | Kind::MutableLocal(id) = &value.kind {
        return constants.get(id).copied();
    }
    if let Kind::Binary(op, left, right) = &value.kind {
        let (left, right) = (integer(left, constants)?, integer(right, constants)?);
        return match op.as_str() {
            "+" => left.checked_add(right),
            "-" => left.checked_sub(right),
            "*" => left.checked_mul(right),
            _ => None,
        };
    }
    None
}
fn invalidate(body: &[Statement], constants: &mut Constants) {
    for statement in body {
        match statement {
            Statement::Assign(
                Value {
                    kind: Kind::MutableLocal(id),
                    ..
                },
                _,
            ) => {
                constants.remove(id);
            }
            Statement::While(_, body)
            | Statement::ForItems(_, _, _, _, body)
            | Statement::For(_, _, body) => invalidate(body, constants),
            Statement::If(_, yes, no) => {
                invalidate(yes, constants);
                invalidate(no, constants);
            }
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => {}
        }
    }
}
fn iterations(
    condition: &Value,
    body: &[Statement],
    constants: &Constants,
) -> Option<(usize, usize)> {
    let Kind::Binary(op, left, right) = &condition.kind else {
        return None;
    };
    let Kind::MutableLocal(id) = left.kind else {
        return None;
    };
    let initial = i128::from(*constants.get(&id)?);
    let limit = i128::from(integer(right, constants)?);
    let mut known = constants.clone();
    let mut step = None;
    let mut increment = None;
    for statement in body {
        if let Statement::Assign(
            Value {
                kind: Kind::MutableLocal(target),
                ..
            },
            value,
        ) = statement
            && *target == id
        {
            if step.is_some() {
                return None;
            }
            let Kind::Binary(sign, left, right) = &value.kind else {
                return None;
            };
            if !matches!(left.kind, Kind::MutableLocal(source) if source == id) {
                return None;
            }
            let delta = i128::from(integer(right, &known)?);
            increment = Some((right, delta));
            step = Some(match sign.as_str() {
                "+" => delta,
                "-" => -delta,
                _ => return None,
            });
            continue;
        }
        invalidate(std::slice::from_ref(statement), &mut known);
        if !known.contains_key(&id) {
            return None;
        }
    }
    if i128::from(integer(right, &known)?) != limit {
        return None;
    }
    let (expression, delta) = increment?;
    if i128::from(integer(expression, &known)?) != delta {
        return None;
    }
    let step = step?;
    let (distance, stride) = match op.as_str() {
        "<" if step > 0 => (limit - initial, step),
        "<=" if step > 0 => (limit - initial + 1, step),
        ">" if step < 0 => (initial - limit, -step),
        ">=" if step < 0 => (initial - limit + 1, -step),
        _ => return None,
    };
    let count = (distance.max(0) + stride - 1) / stride;
    i64::try_from(initial + count * step).ok()?;
    Some((id, usize::try_from(count).ok()?))
}
fn width(
    body: &[Statement],
    source: &impl Fn(&Value) -> String,
    constants: &mut Constants,
) -> Option<String> {
    let mut parts = Vec::new();
    for statement in body {
        let part = match statement {
            Statement::Call(Value {
                kind: Kind::Query(op, _),
                ..
            }) if membership(op) => "1usize".into(),
            Statement::If(_, yes, no) => format!(
                "({}).max({})",
                width(yes, source, &mut constants.clone())?,
                width(no, source, &mut constants.clone())?
            ),
            Statement::ForItems(_, _, _, collection, body)
            | Statement::For(_, collection, body) => {
                format!(
                    "({}).checked_mul({}).ok_or(\"prepared loop width overflow\")?",
                    source(collection),
                    width(body, source, &mut constants.clone())?
                )
            }
            Statement::While(condition, body) => {
                let count = iterations(condition, body, constants);
                if count.is_none() && mutations(body) > 0 {
                    return None;
                }
                let mut inner = constants.clone();
                invalidate(body, &mut inner);
                format!(
                    "{}usize.checked_mul({}).ok_or(\"prepared loop width overflow\")?",
                    count.map_or(1, |(_, count)| count),
                    width(body, source, &mut inner)?
                )
            }
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => "0usize".into(),
        };
        parts.push(part);
        match statement {
            Statement::Let(id, value) | Statement::Var(id, value) => {
                if let Some(value) = integer(value, constants) {
                    constants.insert(*id, value);
                }
            }
            Statement::Assign(
                Value {
                    kind: Kind::MutableLocal(id),
                    ..
                },
                value,
            ) => {
                if let Some(value) = integer(value, constants) {
                    constants.insert(*id, value);
                } else {
                    constants.remove(id);
                }
            }
            Statement::Exit
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::While(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..)
            | Statement::ForItems(..)
            | Statement::For(..)
            | Statement::If(..) => invalidate(std::slice::from_ref(statement), constants),
        }
    }
    Some(parts.into_iter().fold("0usize".into(), |left, right| {
        format!("({left}).checked_add({right}).ok_or(\"prepared mutation width overflow\")?")
    }))
}
/// Bound membership mutations in checked branches, added-element loops and constant induction loops.
/// No expression outside integer literals, retained local constants and arithmetic is evaluated.
pub fn mutation_width(body: &[Statement], source: impl Fn(&Value) -> String) -> Option<String> {
    width(body, &source, &mut Constants::new())
}

/// Maximum syntactic membership mutations in a single handler traversal.
/// Set-element loops inherit their source's publication-width bound in capacity planning.
pub fn mutations(body: &[Statement]) -> usize {
    body.iter()
        .map(|statement| match statement {
            Statement::Call(Value {
                kind: Kind::Query(op, _),
                ..
            }) if membership(op) => 1,
            Statement::If(_, yes, no) => mutations(yes).max(mutations(no)),
            Statement::ForItems(_, _, _, _, body)
            | Statement::For(_, _, body)
            | Statement::While(_, body) => mutations(body),
            Statement::Exit
            | Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::TimedYield(..)
            | Statement::Yield(_)
            | Statement::Call(_)
            | Statement::Assign(..) => 0,
        })
        .sum()
}
