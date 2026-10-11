//! Ordinary constructor locals own canonical reserved slots before any projection or boxing.
use crate::layouts::global_type;
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::{Literal, Ty};
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
fn schema(ty: &Ty) -> Ty {
    crate::layouts::ordinary_storage(ty).unwrap_or_else(|| ty.clone())
}
/// Emit a single ordered write into an independently preflighted canonical owner.
pub(crate) fn local(plan: &Plan, id: usize, value: &Value, key: &str) -> String {
    let entry = plan
        .nodes
        .iter()
        .find_map(|node| node.globals.iter().position(|(k, _)| k == key))
        .unwrap_or_else(|| unreachable!("compiled constructor owner"));
    format!(
        "let local{id}={{let destination=_ctx.global_state().destination(self.global{entry});{}destination}};",
        crate::prepared_hooks::replace(plan, value, "destination")
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
        return format!("PreparedBounds{} {{{args}}}", global_type(&value.ty));
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

fn child_slot(marker: &str, parent: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{marker}>::bind(&mut columns.prepared_list(({parent}).fields()).first().ok_or_else(||hgl_types::NodeError::new(\"unprepared constructor child\"))?.as_slice())"
    )
}
/// Write declared optional/recursive fields while preserving supplied operand order.
pub(crate) fn fields(
    v: &Value,
    slot: &str,
    mut write: impl FnMut(&Value, &str) -> String,
) -> Option<String> {
    let Kind::Construct(fields) = &v.kind else {
        return None;
    };
    let schema = schema(&v.ty);
    let optional = schema
        .structure()
        .map_or(&[][..], |(_, _, optional)| optional);
    let mut code = String::new();
    for i in optional
        .iter()
        .filter(|i| !fields.iter().any(|(id, _)| id == *i))
    {
        crate::append(
            &mut code,
            format_args!("columns.set_list_len(({slot}).fields().{i}.fields(),0);"),
        );
    }
    for (i, value) in fields {
        let target = format!("({slot}).fields().{i}");
        if optional.contains(i) {
            let (_, schema, _) = schema
                .structure()
                .unwrap_or_else(|_| unreachable!("declared optional constructor field"));
            let edge =
                matches!(&schema[*i].1, Ty::Recursive(batch) if batch.definitions().is_empty());
            let marker = global_type(&value.ty);
            let field = if edge {
                format!("hgl_store::Recursive<{marker}>")
            } else {
                marker.clone()
            };
            let child = child_slot(&field, &target);
            let child = if edge {
                child_slot(&marker, &child)
            } else {
                child
            };
            code += &write(value, &child);
            crate::append(
                &mut code,
                format_args!("columns.set_list_len(({target}).fields(),1);"),
            );
        } else {
            code += &write(value, &target);
        }
    }
    Some(code)
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
