//! Cold argument representation for directly emitted ordinary value calls.
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::{Literal, Ty};
/// Reuse an effect-free literal only when no formal-local reference requires owned text.
pub fn argument(
    value: &Value,
    id: usize,
    body: &[Statement],
    emit: impl Fn(&Value) -> String,
) -> String {
    if let Kind::Literal(Literal::Str(text)) = &value.kind
        && !statements(body, Some(id))
    {
        format!("{text:?}")
    } else {
        emit(value)
    }
}
fn statements(body: &[Statement], id: Option<usize>) -> bool {
    body.iter().any(|statement| match statement {
        Statement::Let(_, v)
        | Statement::Var(_, v)
        | Statement::Borrow(_, v, _)
        | Statement::Return(v)
        | Statement::Yield(v)
        | Statement::Call(v) => used(v, id),
        Statement::TimedYield(a, b) | Statement::Assign(a, b) => used(a, id) || used(b, id),
        Statement::If(v, a, b) => used(v, id) || statements(a, id) || statements(b, id),
        Statement::While(v, body)
        | Statement::For(_, v, body)
        | Statement::ForItems(_, _, _, v, body) => used(v, id) || statements(body, id),
        Statement::Exit => false,
    })
}
fn used(value: &Value, id: Option<usize>) -> bool {
    match &value.kind {
        Kind::Local(local) | Kind::MutableLocal(local) => Some(*local) == id,
        Kind::Construct(fields) | Kind::Captured(_, fields) => {
            fields.iter().any(|(_, v)| used(v, id))
        }
        Kind::Delta(parts) => parts.iter().any(|p| p.operands().any(|v| used(v, id))),
        Kind::List(args) | Kind::Native(_, args) | Kind::Query(_, args) => {
            args.iter().any(|v| used(v, id))
        }
        Kind::ValueCall(args, body) => {
            (id.is_none()
                && args.iter().enumerate().any(|(i, v)| {
                    crate::execution_proof::owning_argument(&v.ty)
                        && (!(v.ty == Ty::Str && matches!(v.kind, Kind::Literal(Literal::Str(_))))
                            || statements(body, Some(i)))
                }))
                || args.iter().any(|v| used(v, id))
                || statements(body, id)
        }
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => used(a, id) || used(b, id),
        Kind::Field(v, _)
        | Kind::Length(v)
        | Kind::GlobalSet(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v)
        | Kind::Unary(_, v) => used(v, id),
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::IterationInput(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => false,
    }
}

/// Prove that runtime helper text arguments do not require fresh owning storage.
pub fn prepared(plan: &Plan) -> bool {
    plan.nodes.iter().all(|node| {
        [&node.start, &node.stop]
            .into_iter()
            .chain(node.generator.iter())
            .chain(node.handlers.iter().map(|(_, body)| body))
            .all(|body| !statements(body, None))
            && node
                .handlers
                .iter()
                .all(|(guard, _)| guard.as_ref().is_none_or(|v| !used(v, None)))
    })
}
