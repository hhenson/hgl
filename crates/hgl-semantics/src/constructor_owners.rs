//! Private canonical constructor ownership metadata; admission and evaluation phases are unchanged.
use crate::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
fn hoist(value: &mut Value, configuration: &mut Vec<Value>, owning: &impl Fn(&Ty) -> bool) {
    if value.closed() && !matches!(value.ty, Ty::Family(_)) && owning(&value.ty) {
        let id = configuration.len();
        configuration.push(value.clone());
        value.kind = Kind::Configuration(id);
    } else if let Kind::Construct(fields) = &mut value.kind {
        for (_, child) in fields {
            hoist(child, configuration, owning);
        }
    }
}
fn references(value: &mut Value, owners: &BTreeMap<usize, usize>) -> bool {
    if let Kind::Local(id) | Kind::MutableLocal(id) = value.kind
        && let Some(entry) = owners.get(&id)
    {
        value.kind = Kind::BorrowedLocal(id, *entry, matches!(value.kind, Kind::MutableLocal(_)));
        value.snapshot = false;
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
        let growing = crate::constructor_growth::locals(node);
        let mut owners = BTreeMap::new();
        let mut origins = BTreeMap::<usize, Value>::new();
        for body in std::iter::once(&mut node.start)
            .chain(std::iter::once(&mut node.stop))
            .chain(node.handlers.iter_mut().map(|(_, body)| body))
        {
            crate::ir_visit::nodes_mut(body, &mut |s| {
                if let Statement::Assign(target, value) = s
                    && matches!(target.kind,Kind::MutableLocal(id) if owners.contains_key(&id))
                {
                    hoist(value, &mut node.configuration, &owning);
                }
                if let Statement::Let(id, value) | Statement::Var(id, value) = s
                    && matches!(value.ty, Ty::Struct(..) | Ty::Recursive(_) | Ty::Family(_))
                    && (matches!(value.kind, Kind::Construct(_))
                        || matches!(&value.kind, Kind::Unary(op, _) if op=="family")
                        || matches!(value.kind,Kind::Local(id)|Kind::MutableLocal(id) if origins.contains_key(&id)))
                    && !growing.contains(id)
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
        if !owners.is_empty() {
            node.global_state = true;
        }
        crate::ir_visit::hooks_mut(node, &mut |v| references(v, &owners));
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
    if let Kind::Local(source) | Kind::MutableLocal(source) = value.kind {
        return origins[&source].clone();
    }
    *value = convert(value).unwrap_or_else(|| value.clone());
    hoist(value, configuration, owning);
    value.clone()
}
