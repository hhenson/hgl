//! Callsite-local owner identities keep helper storage separate from caller and sibling scopes.
use crate::ir::{Kind, Plan, Statement, Value};
use std::collections::BTreeMap;
fn declarations(body: &mut [Statement], visit: &mut impl FnMut(&mut usize)) {
    crate::ir_visit::nodes_mut(body, &mut |s| {
        if let Statement::Let(id, _)
        | Statement::Var(id, _)
        | Statement::Borrow(id, _, _)
        | Statement::For(id, _, _) = s
        {
            visit(id);
        } else if let Statement::ForItems(key, child, _, _, _) = s {
            visit(key);
            visit(child);
        }
    });
}
/// Highest declared local ID, excluding nested helper scopes.
pub(crate) fn maximum(body: &mut [Statement]) -> usize {
    let mut next = 0;
    declarations(body, &mut |id| next = next.max(*id + 1));
    next
}
fn references(value: &mut Value, ids: &BTreeMap<usize, usize>) -> bool {
    if let Kind::Local(id)
    | Kind::MutableLocal(id)
    | Kind::ObservedLocal(id)
    | Kind::BorrowedLocal(id, _, _) = &mut value.kind
        && let Some(new) = ids.get(id)
    {
        *id = *new;
    }
    if let Kind::ValueCall(args, _) = &mut value.kind {
        for arg in args {
            crate::ir_visit::value_mut(arg, &mut |v| references(v, ids));
        }
        return false;
    }
    true
}

fn calls(value: &mut Value, next: &mut usize) -> bool {
    let Kind::ValueCall(args, body) = &mut value.kind else {
        return true;
    };
    for arg in args.iter_mut() {
        crate::ir_visit::value_mut(arg, &mut |v| calls(v, next));
    }
    *next = (*next).max(args.len());
    let mut ids = BTreeMap::new();
    declarations(body, &mut |id| {
        let new = *next;
        *next += 1;
        ids.insert(*id, new);
        *id = new;
    });
    crate::ir_visit::statements_mut(body, &mut |v| references(v, &ids));
    crate::ir_visit::statements_mut(body, &mut |v| calls(v, next));
    false
}
/// Give each helper's declared locals independent node-wide storage IDs; formal IDs remain unchanged.
pub fn prepare(plan: &mut Plan) {
    for node in &mut plan.nodes {
        let mut next = 0;
        for body in std::iter::once(&mut node.start)
            .chain(std::iter::once(&mut node.stop))
            .chain(node.handlers.iter_mut().map(|(_, body)| body))
            .chain(node.generator.iter_mut())
        {
            next = next.max(maximum(body));
        }
        crate::ir_visit::hooks_mut(node, &mut |v| calls(v, &mut next));
    }
}
