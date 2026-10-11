//! Constructor frames encode directly into prepared boxes without native aggregate owners.
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
fn argument(value: &Value) -> Option<&Value> {
    if let Kind::Unary(op, child) = &value.kind
        && (op == "any" || op == "family")
    {
        Some(child)
    } else if value.ty == Ty::Any
        && let Kind::Construct(fields) = &value.kind
    {
        fields.first().map(|(_, v)| v)
    } else {
        None
    }
}
fn body(
    plan: &Plan,
    value: &Value,
    prelude: &mut Vec<String>,
    emit: &impl Fn(&Value) -> String,
) -> String {
    if value.ty == Ty::Any {
        let inner = boxed(plan, argument(value).unwrap_or(value), prelude, emit);
        return format!(
            "{{let mut width=hgl_types::any_values::AnyWidth::default();let plan={{let out=&mut width;{inner}}};out.descriptor(plan);{inner};}}"
        );
    }
    if let Kind::List(items) = &value.kind {
        return items
            .iter()
            .map(|v| format!("out.frame(|out|{{{}}});", body(plan, v, prelude, emit)))
            .collect::<Vec<_>>()
            .concat();
    }
    if let Kind::Construct(supplied) = &value.kind
        && let Some((fields, optional)) = hgl_semantics::any_values::fields(&value.ty)
    {
        let children: Vec<_> = supplied
            .iter()
            .map(|(id, v)| (*id, body(plan, v, prelude, emit)))
            .collect();
        return fields
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let child = children
                    .iter()
                    .find(|(id, _)| *id == i)
                    .map(|(_, code)| code.clone());
                let child = if optional.contains(&i) {
                    child.map_or("out.write(&[0]);".into(), |s| {
                        format!("out.write(&[1]);{s}")
                    })
                } else {
                    child.unwrap_or_else(|| unreachable!("checked constructor field"))
                };
                format!("out.frame(|out|{{{child}}});")
            })
            .collect::<Vec<_>>()
            .concat();
    }
    if let Some(code) = crate::observed::text_encoding(value, prelude, emit) {
        return code;
    }
    let id = prelude.len();
    let n = crate::any_values::name(&value.ty);
    if let Some((arena, source)) = crate::source_slots::source(plan, value) {
        prelude.push(format!("let expression_source{id}={source};"));
        format!(
            "any_slots_{n}({},expression_source{id},out);",
            crate::source_slots::columns(arena)
        )
    } else {
        prelude.push(format!("let expression_value{id}={};", emit(value)));
        format!("any_write_{n}(&expression_value{id},out);")
    }
}
fn boxed(
    plan: &Plan,
    value: &Value,
    prelude: &mut Vec<String>,
    emit: &impl Fn(&Value) -> String,
) -> String {
    if let Some(child) = argument(value) {
        return boxed(plan, child, prelude, emit);
    }
    if matches!(&value.kind,Kind::Construct(fields) if value.ty==Ty::Any && fields.is_empty()) {
        return "None::<&'static hgl_types::AnyPlan>".into();
    }
    if matches!(value.ty, Ty::Any | Ty::Family(_)) {
        let id = prelude.len();
        let n = crate::any_values::name(&value.ty);
        if let Some((arena, source)) = crate::source_slots::source(plan, value) {
            prelude.push(format!("let expression_source{id}={source};"));
            return format!(
                "any_box_slots_{n}({},expression_source{id},out)",
                crate::source_slots::columns(arena)
            );
        }
        prelude.push(format!("let expression_value{id}={};", emit(value)));
        return format!("any_box_native_{n}(&expression_value{id},out)");
    }
    let data = body(plan, value, prelude, emit);
    format!("{data}Some(&ANY_{})", crate::any_values::name(&value.ty))
}
/// Compile a finite ordinary constructor into leaf preludes and reserved frame encoding.
pub(crate) fn encoding(
    plan: &Plan,
    value: &Value,
    emit: &impl Fn(&Value) -> String,
    prelude: &mut Vec<String>,
) -> Option<String> {
    let value = if matches!(value.ty, Ty::Family(_)) {
        argument(value)?
    } else {
        value
    };
    if !(matches!(value.kind, Kind::Construct(_) | Kind::List(_)) || value.ty == Ty::Str)
        || value.ty == Ty::Any
    {
        return None;
    }
    Some(boxed(plan, value, prelude, emit))
}
/// Compose completely preflighted constructor frames without temporary native owners.
pub(crate) fn compose(code: &str, destination: &str) -> String {
    format!(
        "{{use hgl_types::any_values::AnyBuffer as _;columns.compose_any({destination},|columns|{{let mut width=hgl_types::any_values::AnyWidth::default();let out=&mut width;let plan={{{code}}};Ok((plan,width.0))}},|columns,out|{{{code};}})?;}}"
    )
}

fn width(value: &Value, bounds: &impl Fn(&Ty) -> String) -> String {
    if let Some(child) = argument(value) {
        return format!("8usize.saturating_add({})", width(child, bounds));
    }
    if let Kind::List(items) = &value.kind {
        return items.iter().fold("0usize".into(), |s, v| {
            format!(
                "{s}.saturating_add(8usize.saturating_add({}))",
                width(v, bounds)
            )
        });
    }
    if let Kind::Construct(supplied) = &value.kind
        && let Some((fields, optional)) = hgl_semantics::any_values::fields(&value.ty)
    {
        return fields
            .iter()
            .enumerate()
            .fold("0usize".into(), |s, (i, _)| {
                let child = supplied
                    .iter()
                    .find(|(id, _)| *id == i)
                    .map_or("0usize".into(), |(_, v)| width(v, bounds));
                format!(
                    "{s}.saturating_add({}usize.saturating_add({child}))",
                    8 + usize::from(optional.contains(&i))
                )
            });
    }
    format!(
        "any_bound_{}(&({}))",
        crate::any_values::name(&value.ty),
        bounds(&value.ty)
    )
}
/// Reserve finite constructor-expression widths without executing any hook or provider.
pub(crate) fn limits(plan: &Plan, bounds: impl Fn(&Ty) -> String, index: usize) -> String {
    let mut result = Vec::new();
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |value| {
            if let Some(child) = argument(value) {
                result.push(format!(
                    "capacity.limit{index}=capacity.limit{index}.max({});",
                    width(child, &bounds)
                ));
            }
            true
        });
    }
    result.concat()
}
