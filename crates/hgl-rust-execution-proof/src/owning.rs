use hgl_rust_ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
fn owning(ty: &Ty) -> bool {
    !matches!(
        ty,
        Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::Enum(_)
            | Ty::Void
    )
}
fn value(plan: &Plan, input: &Value, looping: bool) -> bool {
    match &input.kind {
        Kind::Captured(..) => false,
        Kind::Native(index, args) => {
            (!owning(&input.ty) || plan.natives[*index].name == "hgraph.native::as_str")
                && args.iter().all(|arg| value(plan, arg, looping))
        }
        Kind::ValueCall(args, body) => {
            !owning(&input.ty)
                && args.iter().all(|arg| value(plan, arg, looping))
                && statements(plan, body, looping)
        }
        Kind::Push(parent, item) => {
            !looping && value(plan, parent, looping) && value(plan, item, looping)
        }
        Kind::List(_) if !input.closed() => false,
        Kind::List(args) | Kind::Query(_, args) => args.iter().all(|arg| value(plan, arg, looping)),
        Kind::Construct(fields) => fields.iter().all(|(_, child)| value(plan, child, looping)),
        Kind::Delta(parts) => {
            input.closed()
                && parts
                    .iter()
                    .all(|part| part.operands().all(|child| value(plan, child, looping)))
        }
        Kind::Index(left, right) | Kind::Binary(_, left, right) => {
            value(plan, left, looping) && value(plan, right, looping)
        }
        Kind::Field(child, _)
        | Kind::Length(child)
        | Kind::GlobalSet(_, child)
        | Kind::IsPresent(child)
        | Kind::Present(child)
        | Kind::Unary(_, child) => value(plan, child, looping),
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => true,
    }
}
fn statements(plan: &Plan, body: &[Statement], looping: bool) -> bool {
    body.iter().all(|statement| match statement {
        Statement::Assign(target, source) => {
            !(looping && owning(&target.ty))
                && value(plan, target, looping)
                && value(plan, source, looping)
        }
        Statement::While(condition, body) | Statement::For(_, condition, body) => {
            value(plan, condition, looping) && statements(plan, body, true)
        }
        Statement::If(condition, yes, no) => {
            value(plan, condition, looping)
                && statements(plan, yes, looping)
                && statements(plan, no, looping)
        }
        Statement::Return(source) if hgl_rust_direct_deltas::supported(source) => true,
        Statement::Let(_, source)
        | Statement::Var(_, source)
        | Statement::Borrow(_, source, _)
        | Statement::Return(source)
        | Statement::Yield(source)
        | Statement::Call(source) => value(plan, source, looping),
        Statement::TimedYield(time, payload) => {
            value(plan, time, looping) && value(plan, payload, looping)
        }
        Statement::Exit => true,
    })
}
pub(super) fn finite(plan: &Plan) -> bool {
    plan.nodes.iter().all(|node| {
        statements(plan, &node.start, false)
            && statements(plan, &node.stop, false)
            && node
                .generator
                .as_ref()
                .is_none_or(|body| statements(plan, body, false))
            && node.handlers.iter().all(|(guard, body)| {
                guard.as_ref().is_none_or(|guard| value(plan, guard, false))
                    && statements(plan, body, false)
            })
    })
}
