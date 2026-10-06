//! Pure lexical classification for checking-time ordinary evaluation.
use hgl_rust_ir::{Kind, Statement, Value};
use std::collections::BTreeSet;

/// Whether a complete expression can execute without external values or effects.
pub fn context_free(value: &Value) -> bool {
    expression(value, &BTreeSet::new())
}
fn expression(value: &Value, locals: &BTreeSet<usize>) -> bool {
    match &value.kind {
        Kind::Literal(_) | Kind::Void => true,
        Kind::Local(id) | Kind::MutableLocal(id) => locals.contains(id),
        Kind::List(values) => values.iter().all(|v| expression(v, locals)),
        Kind::Construct(fields) => fields.iter().all(|(_, v)| expression(v, locals)),
        Kind::Delta(parts) => parts
            .iter()
            .all(|part| part.operands().all(|value| expression(value, locals))),
        Kind::Field(value, _) | Kind::Length(value) | Kind::Unary(_, value) => {
            expression(value, locals)
        }
        Kind::Binary(_, a, b) | Kind::Index(a, b) | Kind::Push(a, b) => {
            expression(a, locals) && expression(b, locals)
        }
        Kind::ValueCall(args, body) => {
            args.iter().all(|v| expression(v, locals)) && block(body, &(0..args.len()).collect())
        }
        Kind::TemporalLiteral(_)
        | Kind::Captured(..)
        | Kind::Prepared(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::ObservedLocal(_)
        | Kind::GeneratorLocal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Native(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability
        | Kind::WiringFailure(_) => false,
    }
}
fn block(body: &[Statement], inherited: &BTreeSet<usize>) -> bool {
    let mut locals = inherited.clone();
    body.iter().all(|statement| match statement {
        Statement::Let(id, value) | Statement::Var(id, value) => {
            let closed = expression(value, &locals);
            locals.insert(*id);
            closed
        }
        Statement::Yield(value) | Statement::Call(value) => expression(value, &locals),
        Statement::Assign(target, value) => {
            expression(target, &locals) && expression(value, &locals)
        }
        Statement::If(condition, yes, no) => {
            expression(condition, &locals) && block(yes, &locals) && block(no, &locals)
        }
        Statement::Exit => true,
        Statement::Borrow(..)
        | Statement::For(..)
        | Statement::TimedYield(..)
        | Statement::While(..)
        | Statement::Return(_) => false,
    })
}
