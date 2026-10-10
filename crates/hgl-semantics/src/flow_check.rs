//! Pure guard proof and branch reachability checking.
use crate::ir::{Kind, Statement, Value};
use hgl_source::Expr;
use std::collections::BTreeSet;
/// Whether every reachable branch ends before the following statement.
pub fn terminates(body: &[Statement]) -> bool {
    body.iter().any(|statement| match statement {
        Statement::Return(_) | Statement::Yield(_) | Statement::Exit => true,
        Statement::If(_, yes, no) => terminates(yes) && terminates(no),
        Statement::Let(..)
        | Statement::Call(_)
        | Statement::Var(..)
        | Statement::Borrow(..)
        | Statement::Assign(..)
        | Statement::ForItems(..)
        | Statement::For(..)
        | Statement::While(..)
        | Statement::TimedYield(..) => false,
    })
}
/// Derive endpoint/presence facts from a checked condition and truth branch.
pub fn facts(
    value: &Value,
    truth: bool,
    incoming: &BTreeSet<(String, usize)>,
) -> BTreeSet<(String, usize)> {
    if let Kind::Unary(op, value) = &value.kind
        && op == "!"
    {
        return facts(value, !truth, incoming);
    }
    if let Kind::Binary(op, a, b) = &value.kind
        && matches!(op.as_str(), "&&" | "||")
    {
        let short = op == "||";
        let continuing = facts(b, truth, &facts(a, !short, incoming));
        return if truth == short {
            merge_facts(&facts(a, truth, incoming), &continuing)
        } else {
            continuing
        };
    }
    let mut result = incoming.clone();
    if let Kind::IsPresent(value) = &value.kind
        && let Kind::Local(id) = value.kind
    {
        result.insert((if truth { "present" } else { "absent" }.into(), id));
    }
    if let Kind::Query(op, args) = &value.kind
        && truth
        && (op == "valid" || (op == "modified" && args.len() == 1))
    {
        result.extend(args.iter().filter_map(|v| {
            if let Kind::Input(id, _) = v.kind {
                Some((op.clone(), id))
            } else {
                None
            }
        }));
    }
    result
}
/// Merge facts from alternative reachable control-flow paths.
pub fn merge_facts(
    a: &BTreeSet<(String, usize)>,
    b: &BTreeSet<(String, usize)>,
) -> BTreeSet<(String, usize)> {
    let reachable = |facts: &BTreeSet<(String, usize)>| {
        !facts
            .iter()
            .any(|(kind, id)| kind == "present" && facts.contains(&("absent".into(), *id)))
    };
    if !reachable(a) {
        return b.clone();
    }
    if !reachable(b) {
        return a.clone();
    }
    a.intersection(b).cloned().collect()
}

/// Establish implicit endpoint proofs for a checked temporal handler.
pub fn handler_facts(guard: Option<&Value>, inputs: usize) -> BTreeSet<(String, usize)> {
    if let Some(guard) = guard {
        return facts(guard, true, &BTreeSet::new());
    }
    let mut facts = (0..inputs)
        .map(|i| ("valid".into(), i))
        .collect::<BTreeSet<_>>();
    if inputs == 1 {
        facts.insert(("modified".into(), 0));
    }
    facts
}

/// Apply implicit input readiness only to fully admitted node publications.
pub fn node_guard(node: &crate::ir::Node, expr: Expr) -> Expr {
    if node.inputs.iter().all(|(_, _, ty)| ty.publication()) {
        expr.handler_guard(
            &node
                .inputs
                .iter()
                .map(|(n, _, _)| n.clone())
                .collect::<Vec<_>>(),
        )
    } else {
        expr
    }
}
