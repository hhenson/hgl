//! Cold lexical destination reservation for complete ordinary observations.
use crate::call_locals::{maximum, node_maximum};
use crate::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;

fn scan(
    body: &mut Vec<Statement>,
    locals: &mut std::collections::BTreeMap<usize, Ty>,
    next: &mut usize,
) {
    crate::ir_visit::statements_mut(body, &mut |v| calls(v, locals, next));
    let mut result = Vec::new();
    for mut statement in std::mem::take(body) {
        match &mut statement {
            Statement::Let(id, v) | Statement::Var(id, v) if v.snapshot => {
                locals.insert(*id, v.ty.clone());
            }
            Statement::Return(value)
            | Statement::Assign(_, value)
            | Statement::TimedYield(_, value)
                if value.snapshot
                    && (matches!(value.kind, Kind::Unary(..) | Kind::Construct(_))
                        || value.ty == Ty::Any
                            && !matches!(value.kind, Kind::Local(_) | Kind::MutableLocal(_))) =>
            {
                // The time operand precedes payload retention, including when either fails.
                if let Statement::TimedYield(time, _) = &mut statement {
                    let id = *next;
                    *next += 1;
                    result.push(Statement::Let(id, time.clone()));
                    *time = Value::new(time.ty.clone(), Kind::Local(id));
                }
                let (Statement::Return(value)
                | Statement::Assign(_, value)
                | Statement::TimedYield(_, value)) = &mut statement
                else {
                    unreachable!("snapshot value statement")
                };
                let id = *next;
                *next += 1;
                locals.insert(id, value.ty.clone());
                result.push(Statement::Let(id, value.clone()));
                let mut local = Value::new(value.ty.clone(), Kind::Local(id));
                local.snapshot = true;
                *value = local;
            }
            Statement::If(_, a, b) => {
                scan(a, locals, next);
                scan(b, locals, next);
            }
            Statement::While(_, b)
            | Statement::For(_, _, b)
            | Statement::ForItems(_, _, _, _, b) => scan(b, locals, next),
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
        result.push(statement);
    }
    *body = result;
}
fn calls(
    value: &mut Value,
    locals: &mut std::collections::BTreeMap<usize, Ty>,
    next: &mut usize,
) -> bool {
    if let Kind::ValueCall(args, body) = &mut value.kind {
        for arg in args {
            crate::ir_visit::value_mut(arg, &mut |v| calls(v, locals, next));
        }
        scan(body, locals, next);
        return false;
    }
    true
}

/// Reserve private local destinations in the emission plan; no source type is changed.
pub fn prepare(plan: &mut Plan, storage: impl Fn(&Ty) -> Ty) {
    for (node_id, node) in plan.nodes.iter_mut().enumerate() {
        let mut locals = std::collections::BTreeMap::new();
        let mut next = node_maximum(node);
        crate::ir_visit::hooks_mut(node, &mut |v| {
            if let Kind::ValueCall(_, body) = &mut v.kind {
                next = next.max(maximum(body));
            }
            true
        });
        scan(&mut node.start, &mut locals, &mut next);
        scan(&mut node.stop, &mut locals, &mut next);
        for (_, body) in &mut node.handlers {
            scan(body, &mut locals, &mut next);
        }
        if let Some(body) = &mut node.generator {
            scan(body, &mut locals, &mut next);
        }
        if !locals.is_empty() {
            node.global_state = true;
        }
        for (id, ty) in locals {
            let key = format!("\0snapshot:{node_id}:{id}");
            if !node.globals.iter().any(|(k, _)| k == &key) {
                node.globals.push((key, storage(&ty)));
            }
        }
    }
}
