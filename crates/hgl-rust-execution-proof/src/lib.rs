//! Cold whole-adapter selection without changing source admission or running hooks.
mod owning;
use hgl_rust_ir::{Kind, Plan, Statement, Value};
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
/// Choose finite prepared transport only when membership, owning width and replay count are proved.
/// Unknown plans retain the existing whole-adapter execution path; no hook is evaluated here.
pub fn prepared(plan: &Plan) -> bool {
    owning::finite(plan)
        && plan.nodes.iter().all(|node| {
            node.generator.as_ref().is_none_or(|body| replay(body))
                && std::iter::once(&node.start)
                    .chain(std::iter::once(&node.stop))
                    .chain(node.handlers.iter().map(|(_, body)| body))
                    .all(|body| {
                        hgl_rust_mutation_bounds::mutation_width(body, |_| "1usize".into())
                            .is_some()
                    })
        })
}
