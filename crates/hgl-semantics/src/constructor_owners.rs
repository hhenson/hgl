//! Private canonical constructor ownership metadata; admission and evaluation phases are unchanged.
use crate::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
fn hoist(value: &mut Value, configuration: &mut Vec<Value>, owning: &impl Fn(&Ty) -> bool) {
    if value.closed() && owning(&value.ty) {
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
pub fn prepare(plan: &Plan, owning: impl Fn(&Ty) -> bool) -> Plan {
    let mut plan = plan.clone();
    for (node_id, node) in plan.nodes.iter_mut().enumerate() {
        let mut owners = BTreeMap::new();
        for body in std::iter::once(&mut node.start)
            .chain(std::iter::once(&mut node.stop))
            .chain(node.handlers.iter_mut().map(|(_, body)| body))
        {
            crate::ir_visit::nodes_mut(body, &mut |s| {
                if let Statement::Let(id, value) = s
                    && matches!(value.ty, Ty::Struct(..) | Ty::Recursive(_))
                    && matches!(value.kind, Kind::Construct(_))
                    && owning(&value.ty)
                {
                    let key = format!("\0constructor:{node_id}:{id}");
                    let entry = node.globals.len();
                    node.globals.push((key.clone(), value.ty.clone()));
                    owners.insert(*id, entry);
                    hoist(value, &mut node.configuration, &owning);
                    *value = Value::new(value.ty.clone(), Kind::Query(key, vec![value.clone()]));
                }
            });
        }
        if !owners.is_empty() {
            node.global_state = true;
        }
        crate::ir_visit::hooks_mut(node, &mut |v| references(v, &owners));
        let mut globals = std::mem::take(&mut node.globals);
        let mut configuration = std::mem::take(&mut node.configuration);
        let mut stage = |v: &mut Value| {
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
                let key = format!("\0constructor:{node_id}:expression:{}", globals.len());
                globals.push((key.clone(), v.ty.clone()));
                hoist(v, &mut configuration, &owning);
                *v = Value::new(v.ty.clone(), Kind::Query(key, vec![v.clone()]));
                return false;
            }
            !matches!(v.kind, Kind::Captured(..) | Kind::ValueCall(..))
        };
        for (_, body) in &mut node.handlers {
            crate::ir_visit::statements_mut(body, &mut stage);
        }
        node.global_state |= !globals.is_empty();
        node.globals = globals;
        node.configuration = configuration;
    }
    plan
}
