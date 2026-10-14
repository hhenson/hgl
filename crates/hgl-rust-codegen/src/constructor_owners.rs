//! Ordinary constructor locals own canonical reserved slots before any projection or boxing.
use crate::{constructor_fields::schema, layouts::global_type};
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::{Literal, Ty};
use std::collections::{BTreeMap, BTreeSet};

/// Canonical staging requires present leaves; temporal aggregate captures retain child presence.
pub(crate) fn complete(value: &Value, locals: &BTreeMap<usize, Value>) -> bool {
    complete_value(value, locals, &mut BTreeSet::new())
}
fn complete_value(
    value: &Value,
    locals: &BTreeMap<usize, Value>,
    active: &mut BTreeSet<usize>,
) -> bool {
    if let Kind::Construct(fields) = &value.kind {
        return fields
            .iter()
            .all(|(_, child)| complete_value(child, locals, active));
    }
    if !value.snapshot {
        return true;
    }
    if let Kind::Local(id) | Kind::MutableLocal(id) = value.kind {
        if !active.insert(id) {
            return false;
        }
        let complete = locals
            .get(&id)
            .is_some_and(|v| complete_value(v, locals, active));
        active.remove(&id);
        return complete;
    }
    matches!(&value.kind, Kind::Unary(op, _) if op == "snapshot")
        && !matches!(
            value.ty,
            Ty::Tuple(_) | Ty::Struct(..) | Ty::List(..) | Ty::Map(..)
        )
}
/// Canonical fixed family fields preserve a checked concrete member without native construction.
pub(crate) fn family(value: &Value) -> Option<Value> {
    let (Ty::Family(target), Kind::Unary(op, member)) = (&value.ty, &value.kind) else {
        return None;
    };
    if op != "family" {
        return None;
    }
    let index = target
        .members()
        .iter()
        .position(|(_, ty)| *ty == member.ty)?;
    Some(Value::new(
        value.ty.clone(),
        Kind::Construct(vec![
            (
                0,
                Value::new(
                    Ty::I64,
                    Kind::Literal(Literal::Int(
                        i64::try_from(index).unwrap_or_else(|_| unreachable!("finite family tag")),
                    )),
                ),
            ),
            (index + 1, *member.clone()),
        ]),
    ))
}
/// Emit a single ordered write into an independently preflighted canonical owner.
pub(crate) fn local(plan: &Plan, id: usize, value: &Value, key: &str) -> String {
    format!("let local{id}={};", retained(plan, value, key))
}
/// Recover the selected private destination without executing its initializer.
pub fn destination(plan: &Plan, key: &str) -> String {
    let entry = plan
        .nodes
        .iter()
        .find_map(|node| node.globals.iter().position(|(k, _)| k == key))
        .unwrap_or_else(|| unreachable!("compiled constructor owner"));
    format!("_ctx.global_state().destination(self.global{entry})")
}
fn retained(plan: &Plan, value: &Value, key: &str) -> String {
    format!(
        "{{let destination={};{}destination}}",
        destination(plan, key),
        stage(plan, value, "destination")
    )
}
/// Evaluate an expression's private staging owner without publishing a replacement early.
pub fn source(plan: &Plan, value: &Value) -> Option<String> {
    let Kind::Query(key, args) = &value.kind else {
        return None;
    };
    if !key.starts_with("\0constructor:") {
        return None;
    }
    Some(retained(plan, &args[0], key))
}

/// Retain each operand into a private owner before the next operand executes.
fn stage(plan: &Plan, value: &Value, slot: &str) -> String {
    if let Some(code) = crate::native_results::write(value, slot) {
        return code;
    }
    if let Some(code) = crate::constructor_fields::fields(
        value,
        slot,
        |v, s| stage(plan, v, s),
        crate::prepared_hooks::storage,
    ) {
        return code;
    }
    let mut prelude = Vec::new();
    let code = crate::prepared_hooks::write(plan, value, slot, &mut prelude);
    format!(
        "{{{}{}}}",
        prelude.concat(),
        crate::prepared_hooks::storage(&code)
    )
}

fn optional_bound(ty: &Ty, bound: String) -> String {
    let bound = if matches!(ty, Ty::Recursive(batch) if batch.definitions().is_empty()) {
        format!("Box::new({bound})")
    } else {
        bound
    };
    format!("Some({bound})")
}
fn bounds(value: &Value, fallback: &impl Fn(&Ty) -> String) -> String {
    let schema = schema(&value.ty);
    if let Kind::Construct(supplied) = &value.kind
        && let Ok((_, fields, optional)) = schema.structure()
    {
        let args = fields
            .iter()
            .enumerate()
            .map(|(i, (_, ty))| {
                let child = supplied.iter().find(|(id, _)| *id == i).map(|(_, v)| v);
                let bound = child.map_or_else(
                    || "None".into(),
                    |v| {
                        let bound = bounds(v, fallback);
                        if optional.contains(&i) {
                            optional_bound(ty, bound)
                        } else {
                            bound
                        }
                    },
                );
                format!("field{i}:{bound}")
            })
            .collect::<Vec<_>>()
            .join(",");
        let marker = if matches!(value.ty, Ty::Result(..)) {
            "hgl_store::outcomes::ResultBounds".into()
        } else if value.ty == Ty::Error {
            "hgl_store::outcomes::ErrorBounds".into()
        } else {
            format!("PreparedBounds{}", global_type(&value.ty))
        };
        return format!("{marker} {{{args}}}");
    }
    fallback(&value.ty)
}
/// Derive recursive constructor storage from the checked finite expression shape, never a hook.
pub fn global_bounds(plan: &Plan, key: &str, fallback: impl Fn(&Ty) -> String) -> Option<String> {
    if !key.starts_with("\0constructor:") {
        return None;
    }
    let mut result = None;
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |v| {
            if let Kind::Query(op, args) = &v.kind
                && op == key
            {
                result = Some(bounds(
                    args.last()
                        .unwrap_or_else(|| unreachable!("constructor bounds")),
                    &fallback,
                ));
            }
            true
        });
    }
    result
}

/// Recover the checked finite constructor shape, including copied and boxed family owners.
pub(crate) fn origin(value: &Value) -> Option<&Value> {
    let Kind::Query(op, args) = &value.kind else {
        return None;
    };
    if !op.starts_with("\0constructor:") {
        return None;
    }
    let child = args.last()?;
    if matches!(child.ty, Ty::Family(_))
        && let Kind::Construct(fields) = &child.kind
    {
        return fields.last().map(|(_, child)| child);
    }
    Some(child)
}
