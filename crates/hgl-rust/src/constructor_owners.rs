//! Ordinary constructor locals own canonical reserved slots before any projection or boxing.
use crate::layouts::global_type;
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
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
    if let Kind::Construct(supplied) = &value.kind
        && let Ok((_, fields, optional)) = value.ty.structure()
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
pub(crate) fn global_bounds(
    plan: &Plan,
    key: &str,
    fallback: impl Fn(&Ty) -> String,
) -> Option<String> {
    if !key.starts_with("\0constructor:") {
        return None;
    }
    let mut result = None;
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |v| {
            if let Kind::Query(op, args) = &v.kind
                && op == key
            {
                result = Some(bounds(&args[0], &fallback));
            }
            true
        });
    }
    result
}
