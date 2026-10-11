//! Private canonical constructor ownership metadata; admission and evaluation phases are unchanged.
use crate::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
fn references(value: &mut Value, owners: &BTreeMap<usize, usize>) -> bool {
    if let Kind::Local(id) = value.kind
        && let Some(entry) = owners.get(&id)
    {
        value.kind = Kind::BorrowedLocal(id, *entry, false);
    }
    if let Kind::ValueCall(args, _) = &mut value.kind {
        for argument in args {
            crate::ir_visit::value_mut(argument, &mut |v| references(v, owners));
        }
        return false;
    }
    !matches!(value.kind, Kind::Captured(..))
}
/// Retain existing admitted native constructor locals without evaluating any operand early.
pub fn prepare(
    plan: &Plan,
    owning: impl Fn(&Ty) -> bool,
    convert: impl Fn(&Value) -> Option<Value>,
) -> Plan {
    let mut plan = plan.clone();
    for (node_id, node) in plan.nodes.iter_mut().enumerate() {
        let mut owners = BTreeMap::new();
        let mut origins = BTreeMap::<usize, Value>::new();
        for body in std::iter::once(&mut node.start)
            .chain(std::iter::once(&mut node.stop))
            .chain(node.handlers.iter_mut().map(|(_, body)| body))
        {
            crate::ir_visit::nodes_mut(body, &mut |s| {
                if let Statement::Let(id, value) = s
                    && matches!(value.ty, Ty::Struct(..) | Ty::Recursive(_) | Ty::Family(_))
                    && (matches!(value.kind, Kind::Construct(_))
                        || matches!(&value.kind, Kind::Unary(op, _) if op=="family")
                        || matches!(value.kind,Kind::Local(id) if origins.contains_key(&id)))
                    && owning(&value.ty)
                {
                    let origin =
                        origin(value, &origins, &mut node.configuration, &owning, &convert);
                    origins.insert(*id, origin.clone());
                    let key = format!("\0constructor:{node_id}:{id}");
                    let entry = node.globals.len();
                    node.globals.push((key.clone(), value.ty.clone()));
                    owners.insert(*id, entry);
                    *value = Value::new(
                        value.ty.clone(),
                        Kind::Query(key, vec![value.clone(), origin]),
                    );
                }
            });
        }
        crate::ir_visit::hooks_mut(node, &mut |v| references(v, &owners));
        for (_, body) in &mut node.handlers {
            crate::ir_visit::statements_mut(body, &mut |v| {
                if matches!(&v.kind, Kind::Query(op, _) if op.starts_with("\0constructor:")) {
                    return false;
                }
                if matches!(v.kind, Kind::List(_)) && v.closed() {
                    return false;
                }
                if matches!(v.kind, Kind::Construct(_))
                    && matches!(v.ty, Ty::Struct(..) | Ty::Recursive(_))
                    && owning(&v.ty)
                {
                    let key = format!("\0constructor:{node_id}:expression:{}", node.globals.len());
                    node.globals.push((key.clone(), v.ty.clone()));
                    crate::value_constant::retain_closed(v, &mut node.configuration, &owning);
                    *v = Value::new(v.ty.clone(), Kind::Query(key, vec![v.clone()]));
                    return false;
                }
                !matches!(v.kind, Kind::Captured(..) | Kind::ValueCall(..))
            });
        }
        node.global_state |= !node.globals.is_empty();
    }
    plan
}

fn origin(
    value: &mut Value,
    origins: &BTreeMap<usize, Value>,
    configuration: &mut Vec<Value>,
    owning: &impl Fn(&Ty) -> bool,
    convert: &impl Fn(&Value) -> Option<Value>,
) -> Value {
    if let Kind::Local(source) = value.kind {
        return origins[&source].clone();
    }
    *value = convert(value).unwrap_or_else(|| value.clone());
    if matches!(value.ty, Ty::Family(_)) {
        if let Kind::Construct(fields) = &mut value.kind {
            for (_, child) in fields {
                crate::value_constant::retain_closed(child, configuration, owning);
            }
        }
    } else {
        crate::value_constant::retain_closed(value, configuration, owning);
    }
    value.clone()
}
