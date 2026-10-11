//! Cold admission for private owning Tuple observation destinations.
use hgl_semantics::ir::{Plan, Statement};
fn owning(body: &[Statement]) -> bool {
    body.iter().any(|s| match s {
        Statement::Let(_, v)
        | Statement::Var(_, v)
        | Statement::Return(v)
        | Statement::Yield(v)
        | Statement::Call(v)
        | Statement::Assign(_, v) => v.snapshot,
        Statement::If(_, a, b) => owning(a) || owning(b),
        Statement::While(_, b) | Statement::For(_, _, b) | Statement::ForItems(_, _, _, _, b) => {
            owning(b)
        }
        Statement::Exit | Statement::Borrow(..) | Statement::TimedYield(..) => false,
    })
}
/// Reject private observation storage where finite preparation has not been proved.
fn adapter(plan: Plan, prepared: bool) -> Result<Plan, hgl_source::Issue> {
    if !prepared
        && plan.nodes.iter().any(|n| {
            owning(&n.start) || owning(&n.stop) || n.handlers.iter().any(|(_, b)| owning(b))
        })
    {
        return Err(hgl_source::Issue::typed(
            0..0,
            "owning Tuple observations require finite prepared transport",
        ));
    }
    Ok(plan)
}

/// Standalone graphs have no finite input/recording preparation contract.
pub fn standalone(plan: Plan) -> Result<Plan, hgl_source::Issue> {
    adapter(plan, false)
}
/// Use the existing whole-adapter proof before admitting private observation slots.
pub fn prepared(plan: Plan) -> Result<Plan, hgl_source::Issue> {
    if !crate::value_calls::prepared(&plan) {
        return Err(hgl_source::Issue::typed(
            0..0,
            "owning helper arguments require prepared storage",
        ));
    }
    let proved = crate::execution_proof::prepared(&plan);
    adapter(plan, proved)
}
