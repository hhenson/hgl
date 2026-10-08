//! Cold whole-adapter selection without changing source admission or running hooks.
mod owning;
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Literal;
fn local(value: &Value, id: usize) -> bool {
    matches!(value.kind,Kind::MutableLocal(actual) if actual==id)
}
fn replay(body: &[Statement]) -> bool {
    let [
        Statement::Var(id, initial),
        Statement::While(condition, body),
    ] = body
    else {
        return false;
    };
    if !matches!(initial.kind, Kind::Literal(Literal::Int(0))) {
        return false;
    }
    let Kind::Binary(op, left, right) = &condition.kind else {
        return false;
    };
    let Kind::Length(list) = &right.kind else {
        return false;
    };
    if op != "<" || !local(left, *id) || !matches!(list.kind, Kind::Configuration(_)) {
        return false;
    }
    let [Statement::TimedYield(..), Statement::Assign(target, update)] = body.as_slice() else {
        return false;
    };
    let Kind::Binary(op, left, right) = &update.kind else {
        return false;
    };
    op == "+"
        && local(target, *id)
        && local(left, *id)
        && matches!(right.kind, Kind::Literal(Literal::Int(1)))
}
fn direct(body: &[Statement]) -> Option<usize> {
    body.iter().try_fold(0usize, |count, statement| {
        let arrivals = match statement {
            Statement::Yield(_) | Statement::TimedYield(..) => 1,
            Statement::If(_, yes, no) => direct(yes)?.max(direct(no)?),
            Statement::While(..) | Statement::ForItems(..) | Statement::For(..) => return None,
            Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Return(_)
            | Statement::Call(_)
            | Statement::Assign(..)
            | Statement::Exit => 0,
        };
        count.checked_add(arrivals)
    })
}
/// Total direct source arrivals; replay loops are bounded separately by configuration lengths.
/// Unknown loops or arithmetic overflow leave the complete adapter unproved.
pub fn direct_arrivals(plan: &Plan) -> Option<usize> {
    plan.nodes.iter().try_fold(0usize, |count, node| {
        let arrivals = match &node.generator {
            None => 0,
            Some(body) if replay(body) => 0,
            Some(body) => direct(body)?,
        };
        count.checked_add(arrivals)
    })
}
/// Choose finite prepared transport only when membership, owning width and replay count are proved.
/// Unknown plans retain the existing whole-adapter execution path; no hook is evaluated here.
pub fn prepared(plan: &Plan) -> bool {
    !plan.ordinary_instantiation
        && owning::finite(plan)
        && direct_arrivals(plan).is_some()
        && plan.nodes.iter().all(|node| {
            std::iter::once(&node.start)
                .chain(std::iter::once(&node.stop))
                .chain(node.handlers.iter().map(|(_, body)| body))
                .all(|body| {
                    crate::mutation_bounds::mutation_width(body, |_| "1usize".into()).is_some()
                })
        })
}
